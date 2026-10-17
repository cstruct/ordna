use facet_core::{Def, DefaultSource, Field, Shape, StructKind, Type, UserType};
use facet_reflect::Partial;
use facet_value::{VObject, Value, peek_to_value};

use crate::{ConfigPath, ConfigPathSegment};

/// Fills the fields `value` leaves out with the values `shape` decodes them to, returning
/// the paths of the required fields it leaves out, such as `jwt.ttl_seconds`.
///
/// `facet_value` fills a missing field with its type's `Default` when it has one, so a
/// missing `u16` would decode as `0`. A field is required unless it's an `Option`, a map,
/// list or set (missing means empty), has a `#[facet(default)]`, or its type or the struct
/// it's in has a container `#[facet(default)]`.
///
/// Only the parts of `value` that fit `shape` are walked; mismatches are left to decoding.
pub(crate) fn fill_defaults(shape: &'static Shape, value: &mut Value) -> Vec<ConfigPath> {
    let mut walker = Walker::default();
    walker.walk(shape, value);
    walker.missing
}

#[derive(Default)]
struct Walker {
    path: Vec<ConfigPathSegment>,
    missing: Vec<ConfigPath>,
}

impl Walker {
    fn walk(&mut self, shape: &'static Shape, value: &mut Value) {
        match shape.def {
            Def::Option(option) => {
                if !value.is_null() {
                    self.walk(option.t(), value);
                }
            }
            Def::Pointer(pointer) => {
                if let Some(pointee) = pointer.pointee() {
                    self.walk(pointee, value);
                }
            }
            Def::Map(map) => {
                for (key, entry) in value
                    .as_object_mut()
                    .into_iter()
                    .flat_map(VObject::iter_mut)
                {
                    self.at(key.as_str(), |walker| walker.walk(map.v(), entry));
                }
            }
            Def::List(def) => self.items(def.t(), value),
            Def::Array(def) => self.items(def.t(), value),
            Def::Slice(def) => self.items(def.t(), value),
            Def::Set(def) => self.items(def.t(), value),
            Def::Undefined => match shape.ty {
                Type::User(UserType::Struct(sd)) if sd.kind == StructKind::Struct => {
                    if let Some(object) = value.as_object_mut() {
                        let defaults = shape.has_default_attr().then_some(shape);
                        self.fields(sd.fields, defaults, object);
                    }
                }
                Type::User(UserType::Enum(ed)) => {
                    let Some(tag) = shape.get_tag_attr() else {
                        return;
                    };
                    if shape.is_untagged() || shape.get_content_attr().is_some() {
                        return;
                    }
                    let Some(object) = value.as_object_mut() else {
                        return;
                    };
                    let Some(name) = object.get(tag).and_then(Value::as_string) else {
                        return;
                    };
                    let variant = ed
                        .variants
                        .iter()
                        .find(|variant| variant.effective_name() == name.as_str());
                    if let Some(variant) = variant
                        && variant.data.kind == StructKind::Struct
                    {
                        self.fields(variant.data.fields, None, object);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn items(&mut self, item: &'static Shape, value: &mut Value) {
        let items = value.as_array_mut().map(|array| &mut array[..]);
        for (index, element) in items.unwrap_or_default().iter_mut().enumerate() {
            self.at(&index.to_string(), |walker| walker.walk(item, element));
        }
    }

    /// Walks the `fields` of a struct or variant in `object`. Missing fields are filled from
    /// the `Default` of `defaults`, the struct, if it has a container `#[facet(default)]`.
    fn fields(
        &mut self,
        fields: &'static [Field],
        defaults: Option<&'static Shape>,
        object: &mut VObject,
    ) {
        let mut container_default = None;
        for field in fields {
            if field.is_flattened() {
                self.flattened(field.shape(), object);
                continue;
            }
            let name = field.effective_name();
            let key = match field.alias {
                Some(alias) if !object.contains_key(name) && object.contains_key(alias) => alias,
                _ => name,
            };
            if let Some(entry) = object.get_mut(key) {
                self.at(name, |walker| walker.walk(field.shape(), entry));
                continue;
            }
            let default = match defaults {
                Some(shape) => container_default
                    .get_or_insert_with(|| default_value(shape, None))
                    .as_ref()
                    .and_then(Value::as_object)
                    .and_then(|object| object.get(name).cloned()),
                None if is_required(field) => {
                    self.at(name, |walker| walker.missing.push(walker.config_path()));
                    continue;
                }
                None => default_value(field.shape(), field.default_source()),
            };
            if let Some(default) = default {
                object.insert(name, default);
            }
        }
    }

    /// Walks a flattened field, whose fields are in the same `object` as its siblings'.
    fn flattened(&mut self, shape: &'static Shape, object: &mut VObject) {
        match shape.ty {
            Type::User(UserType::Struct(sd)) if sd.kind == StructKind::Struct => {
                let defaults = shape.has_default_attr().then_some(shape);
                self.fields(sd.fields, defaults, object);
            }
            _ => {
                let mut flattened = Value::from(object.clone());
                self.walk(shape, &mut flattened);
            }
        }
    }

    fn at(&mut self, segment: &str, f: impl FnOnce(&mut Self)) {
        let segment = ConfigPathSegment::parse(segment.to_owned())
            .expect("field names and keys of config paths are valid segments");
        self.path.push(segment);
        f(self);
        self.path.pop();
    }

    fn config_path(&self) -> ConfigPath {
        ConfigPath::new(self.path.clone())
    }
}

fn is_required(field: &Field) -> bool {
    !field.has_default()
        && !field.shape().has_default_attr()
        && !matches!(
            field.shape().def,
            Def::Option(_) | Def::Map(_) | Def::List(_) | Def::Set(_)
        )
}

/// The default of `shape` as a value: from `source` if given, else from the type's
/// `Default`, if it has one and it can be represented as a value.
fn default_value(shape: &'static Shape, source: Option<&DefaultSource>) -> Option<Value> {
    // SAFETY: `shape` is the shape of a real type, from its `Facet` impl.
    let partial = unsafe { Partial::alloc_shape_owned(shape) }.ok()?;
    let partial = match source {
        // SAFETY: a field's default function initializes a value of the field's type.
        Some(DefaultSource::Custom(default)) => unsafe {
            partial.set_from_function(|ptr| {
                default(ptr);
                Ok(())
            })
        },
        Some(DefaultSource::FromTrait) | None => partial.set_default(),
    };
    let value = partial.ok()?.build().ok()?;
    peek_to_value(value.peek()).ok()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use facet::Facet;
    use facet_value::value;

    use super::fill_defaults;

    #[derive(Facet)]
    struct Tls {
        cert: String,
        #[facet(default)]
        verify: bool,
    }

    #[derive(Facet)]
    struct Http {
        port: u16,
        public_url: String,
        tls: Option<Tls>,
        headers: HashMap<String, String>,
        tags: Vec<String>,
        #[facet(default = 30)]
        timeout_seconds: u32,
    }

    #[derive(Facet)]
    #[facet(default)]
    struct Limits {
        rps: u32,
    }

    impl Default for Limits {
        fn default() -> Self {
            Self { rps: 10 }
        }
    }

    #[derive(Facet)]
    #[repr(C)]
    #[facet(tag = "type", rename_all = "snake_case")]
    #[allow(dead_code)]
    enum Backend {
        S3 {
            bucket: String,
            region: Option<String>,
        },
        Local {
            root: String,
        },
    }

    #[derive(Facet)]
    struct App {
        http: Http,
        limits: Limits,
        backends: HashMap<String, Backend>,
    }

    fn missing(mut value: facet_value::Value) -> Vec<String> {
        fill_defaults(App::SHAPE, &mut value)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn required_scalars_are_missing_at_any_depth() {
        assert_eq!(
            missing(value!({ "http": {}, "limits": {} })),
            ["http.port", "http.public_url"]
        );
        assert_eq!(missing(value!({})), ["http"]);
    }

    #[test]
    fn options_collections_and_defaults_may_be_missing() {
        assert!(missing(value!({ "http": { "port": 1, "public_url": "x" } })).is_empty());
    }

    #[test]
    fn present_options_must_be_complete() {
        assert_eq!(
            missing(value!({ "http": { "port": 1, "public_url": "x", "tls": {} } })),
            ["http.tls.cert"]
        );
    }

    #[test]
    fn map_entries_and_enum_variants_are_checked() {
        assert_eq!(
            missing(value!({
                "http": { "port": 1, "public_url": "x" },
                "backends": {
                    "eu": { "type": "s3" },
                    "disk": { "type": "local", "root": "/" },
                },
            })),
            ["backends.eu.bucket"]
        );
    }
}

use facet_core::{Def, Field, Shape, StructKind, Type, UserType};
use facet_value::{VObject, Value};

/// The paths of the required fields of `shape` that `value` leaves out, such as
/// `jwt.ttl_seconds`.
///
/// `facet_value` fills a missing field with its type's `Default` when it has one, so a
/// missing `u16` would decode as `0`. A field is required unless it's an `Option`, a map,
/// list or set (missing means empty), has a `#[facet(default)]`, or its type or the struct
/// it's in has a container `#[facet(default)]`.
///
/// Only the parts of `value` that fit `shape` are checked; mismatches are left to decoding.
pub(crate) fn missing_required_fields(shape: &'static Shape, value: &Value) -> Vec<String> {
    let mut missing = Vec::new();
    check(shape, value, &mut Vec::new(), &mut missing);
    missing
}

fn check(shape: &'static Shape, value: &Value, path: &mut Vec<String>, missing: &mut Vec<String>) {
    match shape.def {
        Def::Option(option) => {
            if !value.is_null() {
                check(option.t(), value, path, missing);
            }
        }
        Def::Pointer(pointer) => {
            if let Some(pointee) = pointer.pointee() {
                check(pointee, value, path, missing);
            }
        }
        Def::Map(map) => {
            for (key, entry) in value.as_object().into_iter().flat_map(VObject::iter) {
                path.push(key.as_str().to_owned());
                check(map.v(), entry, path, missing);
                path.pop();
            }
        }
        Def::List(def) => check_items(def.t(), value, path, missing),
        Def::Array(def) => check_items(def.t(), value, path, missing),
        Def::Slice(def) => check_items(def.t(), value, path, missing),
        Def::Set(def) => check_items(def.t(), value, path, missing),
        Def::Undefined => match shape.ty {
            Type::User(UserType::Struct(sd)) if sd.kind == StructKind::Struct => {
                if let Some(object) = value.as_object() {
                    let defaulted = shape.has_default_attr();
                    check_fields(sd.fields, defaulted, object, path, missing);
                }
            }
            Type::User(UserType::Enum(ed)) => {
                let Some(tag) = shape.get_tag_attr() else {
                    return;
                };
                if shape.is_untagged() || shape.get_content_attr().is_some() {
                    return;
                }
                let Some(object) = value.as_object() else {
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
                    check_fields(variant.data.fields, false, object, path, missing);
                }
            }
            _ => {}
        },
        _ => {}
    }
}

fn check_items(
    item: &'static Shape,
    value: &Value,
    path: &mut Vec<String>,
    missing: &mut Vec<String>,
) {
    let items = value.as_array().map(|array| &array[..]).unwrap_or_default();
    for (index, element) in items.iter().enumerate() {
        path.push(index.to_string());
        check(item, element, path, missing);
        path.pop();
    }
}

fn check_fields(
    fields: &'static [Field],
    defaulted: bool,
    object: &VObject,
    path: &mut Vec<String>,
    missing: &mut Vec<String>,
) {
    for field in fields {
        if field.is_flattened() {
            check(field.shape(), &Value::from(object.clone()), path, missing);
            continue;
        }
        let name = field.effective_name();
        let entry = object
            .get(name)
            .or_else(|| field.alias.and_then(|alias| object.get(alias)));
        path.push(name.to_owned());
        match entry {
            Some(entry) => check(field.shape(), entry, path, missing),
            None if !defaulted && is_required(field) => missing.push(path.join(".")),
            None => {}
        }
        path.pop();
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use facet::Facet;
    use facet_value::value;

    use super::missing_required_fields;

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

    fn missing(value: facet_value::Value) -> Vec<String> {
        missing_required_fields(App::SHAPE, &value)
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

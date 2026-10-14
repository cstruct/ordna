use std::collections::BTreeMap;

use facet_core::{ConstTypeId, Def, Field, Shape, StructKind, Type, UserType};

use crate::{ConfigLayer, ConfigPathPattern, ConfigPathSegment, ConfigPathSegmentPattern};

/// Collects the path pattern and layer of every leaf field of `shape`.
///
/// - Structs, and options of them, are walked into their fields.
/// - Maps with scalar keys add a dynamic segment for the key, then walk their values.
/// - Internally tagged enums whose variants are structs or units provide their tag field,
///   and the fields of every variant, at the enum's path.
/// - Everything else is a leaf, including lists.
///
/// A field is secret when it, or any field it is nested in, is marked sensitive, or when
/// its type contains a sensitive field anywhere (such as a list of structs with a secret).
pub(crate) fn schema_fields(shape: &'static Shape) -> BTreeMap<ConfigPathPattern, ConfigLayer> {
    let mut walker = Walker::default();
    // r[impl schema.layer.public]
    walker.walk(shape, &mut Vec::new(), ConfigLayer::Public);
    walker.fields
}

#[derive(Default)]
struct Walker {
    fields: BTreeMap<ConfigPathPattern, ConfigLayer>,
    /// The shapes being walked, to stop at recursive types.
    stack: Vec<ConstTypeId>,
}

impl Walker {
    fn walk(
        &mut self,
        shape: &'static Shape,
        segments: &mut Vec<ConfigPathSegmentPattern>,
        layer: ConfigLayer,
    ) {
        if self.stack.contains(&shape.id) {
            return self.leaf(shape, segments, layer);
        }
        self.stack.push(shape.id);
        match branch(shape) {
            Some(Branch::Fields(fields)) => self.fields(fields, segments, layer),
            Some(Branch::Option(inner)) => self.walk(inner, segments, layer),
            Some(Branch::Map(value)) => {
                segments.push(ConfigPathSegmentPattern::Dynamic);
                self.walk(value, segments, layer);
                segments.pop();
            }
            Some(Branch::Enum { tag, variants }) => {
                let mut tag_segments = segments.clone();
                tag_segments.push(ConfigPathSegmentPattern::Static(ConfigPathSegment::new(
                    tag,
                )));
                self.insert(ConfigPathPattern::new(tag_segments), layer);
                for fields in variants {
                    self.fields(fields, segments, layer);
                }
            }
            None => self.leaf(shape, segments, layer),
        }
        self.stack.pop();
    }

    fn fields(
        &mut self,
        fields: &'static [Field],
        segments: &mut Vec<ConfigPathSegmentPattern>,
        layer: ConfigLayer,
    ) {
        for field in fields {
            // r[impl schema.fields.names]
            let segment = ConfigPathSegment::new(field.effective_name());
            // r[impl schema.layer.secret]
            // r[impl schema.layer.inherit]
            let layer = if field.is_sensitive() {
                ConfigLayer::Secret
            } else {
                layer
            };
            segments.push(ConfigPathSegmentPattern::Static(segment));
            self.walk(field.shape(), segments, layer);
            segments.pop();
        }
    }

    // r[impl schema.layer.nested-leaves]
    fn leaf(
        &mut self,
        shape: &'static Shape,
        segments: &[ConfigPathSegmentPattern],
        layer: ConfigLayer,
    ) {
        if segments.is_empty() {
            return;
        }
        let layer = if contains_sensitive(shape, &mut Vec::new()) {
            ConfigLayer::Secret
        } else {
            layer
        };
        self.insert(ConfigPathPattern::new(segments.to_vec()), layer);
    }

    /// Fields of several enum variants can share a path; it's secret if any of them is.
    fn insert(&mut self, pattern: ConfigPathPattern, layer: ConfigLayer) {
        let entry = self.fields.entry(pattern).or_insert(layer);
        *entry = (*entry).max(layer);
    }
}

enum Branch {
    Fields(&'static [Field]),
    Option(&'static Shape),
    Map(&'static Shape),
    Enum {
        tag: &'static str,
        variants: Vec<&'static [Field]>,
    },
}

// r[impl schema.fields.leaves]
// r[impl schema.fields.maps]
// r[impl schema.fields.enums]
fn branch(shape: &'static Shape) -> Option<Branch> {
    match shape.def {
        Def::Option(option) => branch(option.t()).map(|_| Branch::Option(option.t())),
        Def::Map(map) if matches!(map.k().def, Def::Scalar) => Some(Branch::Map(map.v())),
        Def::Undefined => match shape.ty {
            Type::User(UserType::Struct(sd)) if sd.kind == StructKind::Struct => {
                Some(Branch::Fields(sd.fields))
            }
            Type::User(UserType::Enum(ed)) => {
                let tag = shape.get_tag_attr()?;
                if shape.is_untagged() || shape.get_content_attr().is_some() {
                    return None;
                }
                let variants = ed
                    .variants
                    .iter()
                    .map(|variant| match variant.data.kind {
                        StructKind::Struct | StructKind::Unit => Some(variant.data.fields),
                        _ => None,
                    })
                    .collect::<Option<_>>()?;
                Some(Branch::Enum { tag, variants })
            }
            _ => None,
        },
        _ => None,
    }
}

/// Whether a value of `shape` contains a sensitive field anywhere.
fn contains_sensitive(shape: &'static Shape, stack: &mut Vec<ConstTypeId>) -> bool {
    if stack.contains(&shape.id) {
        return false;
    }
    stack.push(shape.id);
    let found = match shape.def {
        Def::Option(def) => contains_sensitive(def.t(), stack),
        Def::List(def) => contains_sensitive(def.t(), stack),
        Def::Array(def) => contains_sensitive(def.t(), stack),
        Def::Slice(def) => contains_sensitive(def.t(), stack),
        Def::Set(def) => contains_sensitive(def.t(), stack),
        Def::Map(def) => contains_sensitive(def.k(), stack) || contains_sensitive(def.v(), stack),
        Def::Pointer(def) => def
            .pointee()
            .is_some_and(|pointee| contains_sensitive(pointee, stack)),
        _ => match shape.ty {
            Type::User(UserType::Struct(sd)) => fields_contain_sensitive(sd.fields, stack),
            Type::User(UserType::Enum(ed)) => ed
                .variants
                .iter()
                .any(|variant| fields_contain_sensitive(variant.data.fields, stack)),
            _ => false,
        },
    };
    stack.pop();
    found
}

fn fields_contain_sensitive(fields: &'static [Field], stack: &mut Vec<ConstTypeId>) -> bool {
    fields
        .iter()
        .any(|field| field.is_sensitive() || contains_sensitive(field.shape(), stack))
}

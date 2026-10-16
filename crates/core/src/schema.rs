use std::{collections::BTreeMap, sync::OnceLock};

use facet_core::{Facet, Shape};

use crate::{
    ConfigLayer, ConfigPath, ConfigPathPattern, ConfigPathSegment, engine::tree::schema_fields,
};

/// The fields of a config model, or of a set of config contexts, as path patterns with the
/// layer of each field.
///
/// Struct fields are static segments; map keys are dynamic segments matching any key, as
/// in `backends.*.bucket`.
#[derive(Debug, Clone)]
pub struct Schema {
    shapes: Shapes,
    fields: OnceLock<BTreeMap<ConfigPathPattern, ConfigLayer>>,
}

#[derive(Debug, Clone, Copy)]
enum Shapes {
    One(&'static Shape),
    Union(&'static [&'static Shape]),
}

impl Schema {
    pub const fn new<T: Facet<'static>>() -> Self {
        Self {
            shapes: Shapes::One(T::SHAPE),
            fields: OnceLock::new(),
        }
    }

    // r[impl contexts.schema]
    /// The fields of every one of `shapes`, such as the contexts of a set declared with
    /// `config_contexts!`.
    pub const fn union(shapes: &'static [&'static Shape]) -> Self {
        Self {
            shapes: Shapes::Union(shapes),
            fields: OnceLock::new(),
        }
    }

    /// The pattern and layer of every leaf field of the config model, i.e. the paths
    /// sources can provide values for.
    ///
    /// Secret fields are marked with `#[config(secret)]` (or `#[facet(sensitive)]`), which
    /// also makes every field nested in them secret.
    pub fn fields(&self) -> &BTreeMap<ConfigPathPattern, ConfigLayer> {
        self.fields.get_or_init(|| match self.shapes {
            Shapes::One(shape) => schema_fields(shape),
            Shapes::Union(shapes) => {
                let mut fields = BTreeMap::new();
                for (pattern, layer) in shapes.iter().flat_map(|shape| schema_fields(shape)) {
                    // r[impl contexts.schema.secret]
                    let existing = fields.entry(pattern).or_insert(layer);
                    if layer == ConfigLayer::Secret {
                        *existing = ConfigLayer::Secret;
                    }
                }
                fields
            }
        })
    }

    /// The fields without dynamic segments, such as `jwt.ttl_seconds`.
    pub fn static_fields(&self) -> impl Iterator<Item = ConfigPath> + '_ {
        self.fields()
            .keys()
            .filter_map(ConfigPathPattern::as_static)
    }

    /// The layer of the field `path` matches, or `None` if it matches no field. A path
    /// matching several fields, such as `db.main.host` for both `db.main.host` and
    /// `db.*.host` in a union, is secret if any of them is.
    pub fn layer(&self, path: &ConfigPath) -> Option<ConfigLayer> {
        self.fields()
            .iter()
            .filter(|(pattern, _)| pattern.matches(path))
            .map(|(_, layer)| *layer)
            .max_by_key(|layer| *layer == ConfigLayer::Secret)
    }

    /// Whether `segments` lead towards fields, such as `backends` or `backends.eu-west`
    /// for `backends.*.bucket`.
    pub fn leads_to_fields(&self, segments: &[ConfigPathSegment]) -> bool {
        self.fields()
            .keys()
            .any(|pattern| pattern.leads_to(segments))
    }
}

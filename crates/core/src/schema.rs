use std::{collections::BTreeMap, sync::OnceLock};

use facet_core::{Facet, Shape};

use crate::{
    ConfigLayer, ConfigPath, ConfigPathPattern, ConfigPathSegment, engine::tree::schema_fields,
};

/// The fields of a config model, as path patterns with the layer of each field.
///
/// Struct fields are static segments; map keys are dynamic segments matching any key, as
/// in `backends.*.bucket`.
#[derive(Debug, Clone)]
pub struct Schema {
    shape: &'static Shape,
    fields: OnceLock<BTreeMap<ConfigPathPattern, ConfigLayer>>,
}

impl Schema {
    pub const fn new<T: Facet<'static>>() -> Self {
        Self {
            shape: T::SHAPE,
            fields: OnceLock::new(),
        }
    }

    /// The pattern and layer of every leaf field of the config model, i.e. the paths
    /// sources can provide values for.
    ///
    /// Secret fields are marked with `#[config(secret)]` (or `#[facet(sensitive)]`), which
    /// also makes every field nested in them secret.
    pub fn fields(&self) -> &BTreeMap<ConfigPathPattern, ConfigLayer> {
        self.fields.get_or_init(|| schema_fields(self.shape))
    }

    /// The fields without dynamic segments, such as `jwt.ttl_seconds`.
    pub fn static_fields(&self) -> impl Iterator<Item = ConfigPath> + '_ {
        self.fields()
            .keys()
            .filter_map(ConfigPathPattern::as_static)
    }

    /// The layer of the field `path` matches, or `None` if it matches no field.
    pub fn layer(&self, path: &ConfigPath) -> Option<ConfigLayer> {
        self.fields()
            .iter()
            .find(|(pattern, _)| pattern.matches(path))
            .map(|(_, layer)| *layer)
    }

    /// Whether `segments` lead towards fields, such as `backends` or `backends.eu-west`
    /// for `backends.*.bucket`.
    pub fn leads_to_fields(&self, segments: &[ConfigPathSegment]) -> bool {
        self.fields()
            .keys()
            .any(|pattern| pattern.leads_to(segments))
    }
}

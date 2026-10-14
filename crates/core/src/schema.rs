use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use facet_core::{Facet, Shape};

use crate::{ConfigLayer, ConfigPath, engine::tree::schema_fields};

#[derive(Debug, Clone)]
pub struct Schema {
    shape: &'static Shape,
    layers: OnceLock<BTreeMap<ConfigPath, ConfigLayer>>,
}

impl Schema {
    pub const fn new<T: Facet<'static>>() -> Self {
        Self {
            shape: &T::SHAPE,
            layers: OnceLock::new(),
        }
    }

    /// The config paths of every leaf field of the config model, i.e. the paths sources
    /// can provide values for.
    pub fn fields(&self) -> BTreeSet<ConfigPath> {
        self.layers().keys().cloned().collect()
    }

    /// The layer of the field at `path`, or `None` if the schema has no such field.
    ///
    /// Secret fields are marked with `#[config(secret)]` (or `#[facet(sensitive)]`), which
    /// also makes every field nested in them secret.
    pub fn layer(&self, path: &ConfigPath) -> Option<ConfigLayer> {
        self.layers().get(path).copied()
    }

    pub(crate) fn layers(&self) -> &BTreeMap<ConfigPath, ConfigLayer> {
        self.layers.get_or_init(|| schema_fields(self.shape))
    }
}

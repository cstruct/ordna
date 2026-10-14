pub(crate) mod binding;
pub(crate) mod field_candidate;
pub mod registration;
mod settings;

use std::fmt::Display;

pub use crate::source::field_candidate::FieldCandidate;
pub use crate::source::settings::{ConfigLayer, ConfigSourceSettings};
use crate::{ConfigPath, Schema, source::field_candidate::SourceValueDetail};

#[derive(Debug, Clone, Copy)]
pub struct SourceKind(&'static str);

impl Display for SourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait ConfigSource: Send + Sync {
    type Error: Sized + Send + Sync + 'static;
    /// Describes where a value came from within the source, such as the variable name or
    /// file and key. Mapped into the config's provenance type when the source is registered.
    type Detail: SourceValueDetail;

    fn settings(&self) -> ConfigSourceSettings;
    fn load(
        &self,
        schema: &'static Schema,
    ) -> Result<Vec<FieldCandidate<Self::Detail>>, Self::Error>;
    fn kind(&self) -> SourceKind {
        SourceKind(std::any::type_name::<Self>())
    }
}

#[derive(Clone, Debug)]
pub struct SourceMeta {
    pub name: Option<String>,
    pub kind: SourceKind,
    pub settings: ConfigSourceSettings,
}

pub trait SourceFieldPath {
    fn field_path(&self) -> &ConfigPath;
    fn source_kind(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

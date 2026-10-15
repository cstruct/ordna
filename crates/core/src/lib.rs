extern crate self as ordna_core;

mod builder;
mod engine;
mod error;
mod issue;
mod model;
mod path;
mod provenance;
mod schema;
mod source;
mod values;

pub use builder::{ConfigBuilder, LoadedConfig};
pub use error::{ConfigLoadError, ConfigViolations, DecodeError, SourceLoadError};
pub use issue::{ConflictIssue, DiagnosticValue, LayerMismatchIssue, UnknownPathIssue};
pub use model::ConfigModel;
pub use path::{
    ConfigPath, ConfigPathPattern, ConfigPathSegment, ConfigPathSegmentPattern, InvalidPathSegment,
};
pub use provenance::{ConfigValueSource, SourceResolution};
pub use schema::Schema;
pub use source::field_candidate::SourceValueDetail;
pub use source::registration::SourceRegistration;
pub use source::{
    ConfigLayer, ConfigSource, ConfigSourceSettings, FieldCandidate, SourceFieldPath, SourceKind,
    SourceMeta,
};

pub use values::{ValueDetail, ValuesSource};

pub use ordna_macros::{config, config_tree};

#[doc(hidden)]
pub mod __private {
    pub use facet;
}

#[cfg(test)]
mod macro_tests;

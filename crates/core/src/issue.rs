use crate::{ConfigLayer, ConfigPath, source::SourceMeta};

#[derive(Debug, Clone)]
pub struct ConflictIssue {
    pub path: ConfigPath,
    pub source: SourceMeta,
    pub contenders: Vec<DiagnosticValue>,
}

/// A source provided a value for a field of another layer: a public source for a secret
/// field, or a secret source for a public field.
#[derive(Debug, Clone)]
pub struct LayerMismatchIssue {
    pub path: ConfigPath,
    pub source: SourceMeta,
    pub field_layer: ConfigLayer,
    pub value: DiagnosticValue,
}

#[derive(Debug, Clone)]
pub struct UnknownPathIssue {
    pub path: ConfigPath,
    pub source: SourceMeta,
    pub value: DiagnosticValue,
}

#[derive(Debug, Clone)]
pub enum DiagnosticValue {
    Text(String),
    PartiallyRedacted(String),
    FullyRedacted,
    Opaque { type_name: &'static str },
    Unknown,
}

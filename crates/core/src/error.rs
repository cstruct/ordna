use crate::{
    ConfigPath, PartialConfig, SourceFieldPath,
    issue::{ConflictIssue, DiagnosticValue, LayerMismatchIssue, UnknownPathIssue},
    source::SourceMeta,
};

#[derive(Clone, Debug)]
pub enum ConfigLoadError<E> {
    Source(SourceLoadError<E>),
    Violations(ConfigViolations),
}

/// Loading failed, with what the load got to when it failed with violations.
#[derive(Clone, Debug)]
pub enum PartialLoadError<P: SourceFieldPath, E> {
    Source(SourceLoadError<E>),
    Violations(Box<PartialConfig<P>>),
}

impl<P: SourceFieldPath, E> From<PartialLoadError<P, E>> for ConfigLoadError<E> {
    fn from(error: PartialLoadError<P, E>) -> Self {
        match error {
            PartialLoadError::Source(error) => Self::Source(error),
            PartialLoadError::Violations(partial) => Self::Violations(partial.into_violations()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SourceLoadError<E> {
    pub source: SourceMeta,
    pub error: E,
}

#[derive(Clone, Debug, Default)]
pub struct ConfigViolations {
    conflicts: Vec<ConflictIssue>,
    unknown_paths: Vec<UnknownPathIssue>,
    layer_mismatches: Vec<LayerMismatchIssue>,
    decode_errors: Vec<DecodeError>,
    missing_fields: Vec<MissingField>,
    // refine_errors: Vec<RefineError>,
}

impl ConfigViolations {
    pub(crate) fn add_conflict_violation(&mut self, conflict: ConflictIssue) {
        self.conflicts.push(conflict);
    }

    pub(crate) fn add_unknown_path_violation(
        &mut self,
        path: ConfigPath,
        source: SourceMeta,
        value: DiagnosticValue,
    ) {
        self.unknown_paths.push(UnknownPathIssue {
            path,
            source,
            value,
        });
    }

    pub(crate) fn add_layer_mismatch_violation(&mut self, issue: LayerMismatchIssue) {
        self.layer_mismatches.push(issue);
    }

    pub(crate) fn add_decode_violation(&mut self, error: DecodeError) {
        self.decode_errors.push(error);
    }

    pub(crate) fn add_missing_field_violation(&mut self, field: MissingField, error: DecodeError) {
        self.missing_fields.push(field);
        self.decode_errors.push(error);
    }

    /// Takes the violations of `other`.
    pub(crate) fn extend(&mut self, other: ConfigViolations) {
        self.conflicts.extend(other.conflicts);
        self.unknown_paths.extend(other.unknown_paths);
        self.layer_mismatches.extend(other.layer_mismatches);
        self.decode_errors.extend(other.decode_errors);
        self.missing_fields.extend(other.missing_fields);
    }

    pub fn has_violations(&self) -> bool {
        !self.conflicts.is_empty()
            || !self.unknown_paths.is_empty()
            || !self.layer_mismatches.is_empty()
            || !self.decode_errors.is_empty()
    }

    pub fn conflicts(&self) -> &[ConflictIssue] {
        &self.conflicts
    }

    pub fn unknown_paths(&self) -> &[UnknownPathIssue] {
        &self.unknown_paths
    }

    pub fn layer_mismatches(&self) -> &[LayerMismatchIssue] {
        &self.layer_mismatches
    }

    /// The decode violations, among them one for every missing field.
    pub fn decode_errors(&self) -> &[DecodeError] {
        &self.decode_errors
    }

    /// The required fields no source set. Each is also a decode violation.
    pub fn missing_fields(&self) -> &[MissingField] {
        &self.missing_fields
    }
}

/// A required field no source set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingField {
    /// The context the field is required by, when loading contexts.
    pub context: Option<&'static str>,
    /// The path of the field, such as `jwt.ttl_seconds`, or `backends.eu.bucket` for a
    /// field of a map entry.
    pub path: ConfigPath,
}

/// The merged source values could not be decoded into the config model.
#[derive(Clone, Debug)]
pub struct DecodeError {
    pub message: String,
}

use crate::{
    ConfigPath,
    issue::{ConflictIssue, DiagnosticValue, LayerMismatchIssue, UnknownPathIssue},
    source::SourceMeta,
};

#[derive(Clone, Debug)]
pub enum ConfigLoadError<E> {
    Source(SourceLoadError<E>),
    Violations(ConfigViolations),
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

    pub fn decode_errors(&self) -> &[DecodeError] {
        &self.decode_errors
    }
}

/// The merged source values could not be decoded into the config model.
#[derive(Clone, Debug)]
pub struct DecodeError {
    pub message: String,
}

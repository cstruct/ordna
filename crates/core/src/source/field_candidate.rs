use std::fmt::Debug;

use facet_value::Value;

use crate::{ConfigLayer, ConfigPath, issue::DiagnosticValue, source::SourceMeta};

/// A value a source provides for the field at `path`, with a `detail` describing where in
/// the source it came from.
#[derive(Debug, Clone)]
pub struct FieldCandidate<D> {
    path: ConfigPath,
    value: Value,
    detail: D,
}

impl<D> FieldCandidate<D> {
    pub fn new(path: ConfigPath, value: Value, detail: D) -> Self {
        Self {
            path,
            value,
            detail,
        }
    }

    pub fn path(&self) -> &ConfigPath {
        &self.path
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn detail(&self) -> &D {
        &self.detail
    }

    pub(crate) fn map_detail<P>(self, map: impl FnOnce(ConfigPath, D) -> P) -> FieldCandidate<P> {
        let detail = map(self.path.clone(), self.detail);
        FieldCandidate {
            path: self.path,
            value: self.value,
            detail,
        }
    }

    pub(crate) fn into_detail(self) -> D {
        self.detail
    }

    // r[impl load.diagnostics.redact]
    /// The value as shown in diagnostics. Values are redacted when either the source or the
    /// field they target (`field_layer`) is secret.
    pub(crate) fn as_diagnostic_value(
        &self,
        source_meta: &'_ SourceMeta,
        field_layer: ConfigLayer,
    ) -> DiagnosticValue {
        if source_meta.settings.layer.max(field_layer) == ConfigLayer::Public {
            if let Some(inner) = self.value.as_string() {
                DiagnosticValue::Text(inner.as_str().into())
            } else if let Some(inner) = self.value.as_bool() {
                DiagnosticValue::Text(inner.to_string())
            } else if let Some(inner) = self.value.as_number() {
                if let Some(inner) = inner.to_f32() {
                    DiagnosticValue::Text(inner.to_string())
                } else if let Some(inner) = inner.to_f64() {
                    DiagnosticValue::Text(inner.to_string())
                } else if let Some(inner) = inner.to_i32() {
                    DiagnosticValue::Text(inner.to_string())
                } else if let Some(inner) = inner.to_i64() {
                    DiagnosticValue::Text(inner.to_string())
                } else if let Some(inner) = inner.to_u32() {
                    DiagnosticValue::Text(inner.to_string())
                } else if let Some(inner) = inner.to_u64() {
                    DiagnosticValue::Text(inner.to_string())
                } else {
                    DiagnosticValue::Unknown
                }
            } else {
                DiagnosticValue::Unknown
            }
        } else {
            DiagnosticValue::FullyRedacted
        }
    }
}

pub trait SourceValueDetail: Debug + Send + Sync + 'static {
    fn label(&self) -> String;
    fn help(&self) -> Option<String> {
        None
    }
}

//! Config contexts: config models loaded from the sources of a shared set, each entrypoint
//! decoding only the contexts it needs.

use std::collections::BTreeMap;

use facet_core::Facet;
use facet_value::{VObject, Value, from_value};

use crate::{
    ConfigPath, ConfigSchema, FieldCandidate, Schema,
    engine::{insert_nested, required},
    error::{ConfigViolations, DecodeError, MissingField},
};

// r[impl contexts.membership]
/// A config model that is one of the contexts of the set `C`, declared with
/// `config_contexts!`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a context of `{C}`",
    label = "not in the context set",
    note = "add `{Self}` to the `config_contexts!` declaring `{C}`"
)]
pub trait ConfigContext<C>: ConfigSchema + Facet<'static> {}

/// Contexts of the set `C` loaded together, as a tuple such as `(Http, Db)`.
pub trait ConfigContexts<C>: Sized {
    #[doc(hidden)]
    fn decode<P>(decoder: &mut ContextDecoder<'_, P>) -> Option<Self>;
}

/// Decodes contexts from the winning values of a load, collecting their violations.
#[doc(hidden)]
pub struct ContextDecoder<'a, P> {
    candidates: &'a BTreeMap<ConfigPath, Vec<FieldCandidate<P>>>,
    violations: &'a mut ConfigViolations,
    schemas: Vec<&'static Schema>,
    values: Vec<Value>,
}

impl<'a, P> ContextDecoder<'a, P> {
    pub(crate) fn new(
        candidates: &'a BTreeMap<ConfigPath, Vec<FieldCandidate<P>>>,
        violations: &'a mut ConfigViolations,
    ) -> Self {
        Self {
            candidates,
            violations,
            schemas: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Decodes context `T`, naming it in its violations.
    pub(crate) fn decode<T: ConfigSchema + Facet<'static>>(&mut self) -> Option<T> {
        self.decode_with(T::schema(), Some(T::SHAPE.type_identifier))
    }

    // r[impl load.decode]
    // r[impl load.decode.strings]
    // r[impl contexts.decode]
    /// Decodes `T` from the values of the fields of `schema`, recording decode violations,
    /// prefixed with `context` if given, if it fails.
    pub(crate) fn decode_with<T: Facet<'static>>(
        &mut self,
        schema: &'static Schema,
        context: Option<&'static str>,
    ) -> Option<T> {
        self.schemas.push(schema);
        let mut root = VObject::new();
        for (path, contenders) in self.candidates {
            if schema.layer(path).is_some() {
                let winner = contenders.last().expect("entries always hold a candidate");
                insert_nested(&mut root, path, winner.value().clone());
            }
        }
        let error = |message: String| DecodeError {
            message: match context {
                Some(context) => format!("{context}: {message}"),
                None => message,
            },
        };
        let root = Value::from(root);
        // r[impl load.partial.defaults]
        let mut filled = root.clone();
        let missing = required::fill_defaults(T::SHAPE, &mut filled);
        self.values.push(filled);
        if !missing.is_empty() {
            // r[impl load.violations.missing]
            for path in missing {
                let message = error(format!("missing required field `{path}`"));
                self.violations
                    .add_missing_field_violation(MissingField { context, path }, message);
            }
            return None;
        }
        from_value::<T>(root)
            .map_err(|decode_error| {
                self.violations
                    .add_decode_violation(error(decode_error.to_string()))
            })
            .ok()
    }

    /// The schemas of the decoded contexts, and their values with defaults filled in.
    pub(crate) fn into_parts(self) -> (Vec<&'static Schema>, Vec<Value>) {
        (self.schemas, self.values)
    }
}

macro_rules! tuple_contexts {
    ($($context:ident),+) => {
        impl<C, $($context: ConfigContext<C>),+> ConfigContexts<C> for ($($context,)+) {
            #[allow(non_snake_case, reason = "the values are named after their types")]
            fn decode<P>(decoder: &mut ContextDecoder<'_, P>) -> Option<Self> {
                // r[impl contexts.decode.collect]
                // Every context is decoded before failing, to report all their violations.
                let ($($context,)+) = ($(decoder.decode::<$context>(),)+);
                Some(($($context?,)+))
            }
        }
    };
}

tuple_contexts!(A);
tuple_contexts!(A, B);
tuple_contexts!(A, B, C2);
tuple_contexts!(A, B, C2, D);
tuple_contexts!(A, B, C2, D, E);
tuple_contexts!(A, B, C2, D, E, F);
tuple_contexts!(A, B, C2, D, E, F, G);
tuple_contexts!(A, B, C2, D, E, F, G, H);

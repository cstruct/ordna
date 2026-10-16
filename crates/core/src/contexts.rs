//! Config contexts: config models loaded from the sources of a shared set, each entrypoint
//! decoding only the contexts it needs.

use std::collections::BTreeMap;

use facet_core::Facet;
use facet_value::{VObject, Value, from_value};

use crate::{
    ConfigPath, ConfigSchema, FieldCandidate, Schema,
    engine::{insert_nested, required},
    error::{ConfigViolations, DecodeError},
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
        }
    }

    // r[impl load.decode]
    // r[impl load.decode.strings]
    // r[impl contexts.decode]
    /// Decodes `T` from the values of its own fields, recording a decode violation if it
    /// fails.
    pub(crate) fn decode<T: ConfigSchema + Facet<'static>>(&mut self) -> Option<T> {
        let schema = T::schema();
        self.schemas.push(schema);
        let mut root = VObject::new();
        for (path, contenders) in self.candidates {
            if schema.layer(path).is_some() {
                let winner = contenders.last().expect("entries always hold a candidate");
                insert_nested(&mut root, path, winner.value().clone());
            }
        }
        let root = Value::from(root);
        let missing = required::missing_required_fields(T::SHAPE, &root);
        if !missing.is_empty() {
            for path in missing {
                self.violations.add_decode_violation(DecodeError {
                    model: T::SHAPE.type_identifier,
                    message: format!("missing required field `{path}`"),
                });
            }
            return None;
        }
        from_value::<T>(root)
            .map_err(|error| {
                self.violations.add_decode_violation(DecodeError {
                    model: T::SHAPE.type_identifier,
                    message: error.to_string(),
                });
            })
            .ok()
    }

    /// The schemas of the decoded contexts.
    pub(crate) fn into_schemas(self) -> Vec<&'static Schema> {
        self.schemas
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

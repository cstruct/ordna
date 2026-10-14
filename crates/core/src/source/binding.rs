use crate::{
    ConfigLoadError, ConfigSource, FieldCandidate, Schema, SourceKind, SourceLoadError,
    source::{ConfigSourceSettings, SourceMeta, registration::SourceRegistration},
};

pub(crate) trait ErasedConfigSource<P, E>: Send + Sync {
    fn load(&self, schema: &'static Schema) -> Result<Vec<FieldCandidate<P>>, E>;
}

/// A source bound to a config, with its errors and value details mapped into the config's
/// error and provenance types.
pub(crate) struct BoundSource<P, E> {
    pub(crate) meta: SourceMeta,
    source: Box<dyn ErasedConfigSource<P, E>>,
}

struct MappedSource<S: ConfigSource, P, E> {
    source: S,
    registration: SourceRegistration<S, P, E>,
}

impl<S, P, E> ErasedConfigSource<P, E> for MappedSource<S, P, E>
where
    S: ConfigSource,
{
    fn load(&self, schema: &'static Schema) -> Result<Vec<FieldCandidate<P>>, E> {
        let candidates = self
            .source
            .load(schema)
            .map_err(&self.registration.error_mapper)?;
        Ok(candidates
            .into_iter()
            // r[impl provenance.mapping]
            .map(|candidate| candidate.map_detail(&self.registration.origin_mapper))
            .collect())
    }
}

impl<P, E> BoundSource<P, E> {
    pub(crate) fn new_anonymous<S>(source: S, registration: SourceRegistration<S, P, E>) -> Self
    where
        S: ConfigSource + 'static,
        P: 'static,
        E: 'static,
    {
        Self::new(source, registration, None)
    }

    pub(crate) fn new_named<S>(
        name: String,
        source: S,
        registration: SourceRegistration<S, P, E>,
    ) -> Self
    where
        S: ConfigSource + 'static,
        P: 'static,
        E: 'static,
    {
        Self::new(source, registration, Some(name))
    }

    fn new<S>(source: S, registration: SourceRegistration<S, P, E>, name: Option<String>) -> Self
    where
        S: ConfigSource + 'static,
        P: 'static,
        E: 'static,
    {
        let kind: SourceKind = source.kind();
        let settings: ConfigSourceSettings = source.settings();
        Self {
            meta: SourceMeta {
                name,
                kind,
                settings,
            },
            source: Box::new(MappedSource {
                source,
                registration,
            }),
        }
    }

    // r[impl load.source.error]
    pub(crate) fn load(
        &self,
        schema: &'static Schema,
    ) -> Result<Vec<FieldCandidate<P>>, ConfigLoadError<E>> {
        self.source.load(schema).map_err(|error| {
            ConfigLoadError::Source(SourceLoadError {
                source: self.meta.clone(),
                error,
            })
        })
    }
}

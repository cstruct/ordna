use crate::{ConfigPath, ConfigSource};

// r[impl provenance.mapping]
/// Registers a source type `S` with a config: maps the source's errors into the config's
/// error type `E`, and each value's detail into the config's provenance type `P`.
pub struct SourceRegistration<S, P, E>
where
    S: ConfigSource,
{
    pub(crate) error_mapper: Box<dyn Fn(S::Error) -> E + Send + Sync + 'static>,
    pub(crate) origin_mapper: Box<dyn Fn(ConfigPath, S::Detail) -> P + Send + Sync + 'static>,
}

impl<S, P, E> SourceRegistration<S, P, E>
where
    S: ConfigSource,
{
    /// `origin_mapper` receives the config path a value is for and the source's detail of
    /// where it came from.
    pub fn new(
        error_mapper: impl Fn(S::Error) -> E + Send + Sync + 'static,
        origin_mapper: impl Fn(ConfigPath, S::Detail) -> P + Send + Sync + 'static,
    ) -> Self {
        Self {
            error_mapper: Box::new(error_mapper),
            origin_mapper: Box::new(origin_mapper),
        }
    }
}

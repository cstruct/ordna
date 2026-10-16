use facet_core::Facet;

use crate::{
    ConfigPath, ConfigSource, Schema, SourceFieldPath, source::registration::SourceRegistration,
};

/// The fields sources may provide values for: those of a config model, or of every context
/// of a set declared with `config_contexts!`.
pub trait ConfigSchema {
    fn schema() -> &'static Schema;
}

/// A config structure that can be loaded from sources.
///
/// Usually implemented with the `#[config]` attribute or the `config_tree!` macro.
pub trait ConfigModel<'facet, P, E>: Facet<'facet> + ConfigSchema + Sized
where
    P: SourceFieldPath,
    E: Send + Sync + 'static,
{
    /// Registers source type `S`, mapping its errors into `E` and its value details into
    /// the provenance type `P`. See [`SourceRegistration::new`].
    fn register_source<S>(
        error_mapper: impl Fn(S::Error) -> E + Send + Sync + 'static,
        origin_mapper: impl Fn(ConfigPath, S::Detail) -> P + Send + Sync + 'static,
    ) -> SourceRegistration<S, P, E>
    where
        S: ConfigSource,
    {
        SourceRegistration::new(error_mapper, origin_mapper)
    }
}

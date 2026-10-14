use facet_core::Facet;

use crate::{
    ConfigPath, ConfigSource, Schema, SourceFieldPath, source::registration::SourceRegistration,
};

/// A config structure that can be loaded from sources.
///
/// Usually implemented with the `#[config]` attribute or the `config!` macro.
pub trait ConfigModel<'facet, P, E>: Facet<'facet> + Sized
where
    P: SourceFieldPath,
    E: Send + Sync + 'static,
{
    fn schema() -> &'static Schema;

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

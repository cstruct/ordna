use std::{collections::BTreeMap, marker::PhantomData};

use crate::{
    ConfigLoadError, ConfigModel, ConfigPath, ConfigSource, ConfigValueSource, SourceFieldPath,
    SourceResolution,
    engine::load_config_from_sources,
    source::{binding::BoundSource, registration::SourceRegistration},
};

struct ConfigBuilderInner<'facet, T, P, E>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'facet, P, E>,
    P: SourceFieldPath,
{
    sources: Vec<BoundSource<P, E>>,
    _marker: PhantomData<&'facet T>,
}

pub struct ConfigBuilder<'facet, T, P, E>(ConfigBuilderInner<'facet, T, P, E>)
where
    E: Send + Sync + 'static,
    T: ConfigModel<'facet, P, E>,
    P: SourceFieldPath;
pub struct ConfigBuilderNamed<'facet, T, P, E>(ConfigBuilderInner<'facet, T, P, E>)
where
    E: Send + Sync + 'static,
    T: ConfigModel<'facet, P, E>,
    P: SourceFieldPath;
pub struct ConfigBuilderAnonymous<'facet, T, P, E>(ConfigBuilderInner<'facet, T, P, E>)
where
    E: Send + Sync + 'static,
    T: ConfigModel<'facet, P, E>,
    P: SourceFieldPath;

impl<'facet, T, P, E> ConfigBuilderInner<'facet, T, P, E>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'facet, P, E>,
    P: SourceFieldPath,
{
    fn new() -> Self {
        Self {
            sources: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn source<S>(mut self, source_type_token: SourceRegistration<S, P, E>, source: S) -> Self
    where
        S: ConfigSource + 'static,
        P: 'static,
    {
        self.sources
            .push(BoundSource::new_anonymous(source, source_type_token));
        self
    }

    pub fn source_named<S>(
        mut self,
        name: impl Into<String>,
        source_type_token: SourceRegistration<S, P, E>,
        source: S,
    ) -> Self
    where
        S: ConfigSource + 'static,
        P: 'static,
    {
        self.sources.push(BoundSource::new_named(
            name.into(),
            source,
            source_type_token,
        ));
        self
    }
}

impl<'facet, T, P, E> Default for ConfigBuilder<'facet, T, P, E>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'facet, P, E>,
    P: SourceFieldPath,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'facet, T, P, E> ConfigBuilder<'facet, T, P, E>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'facet, P, E>,
    P: SourceFieldPath,
{
    /// A builder without sources. Add sources with either [`Self::source`] or
    /// [`Self::source_named`]; anonymous and named sources can't be mixed.
    ///
    /// Sources are applied in the order they're added, so later sources shadow earlier ones.
    pub fn new() -> Self {
        Self(ConfigBuilderInner::new())
    }

    pub fn source<S>(
        self,
        source_type_token: SourceRegistration<S, P, E>,
        source: S,
    ) -> ConfigBuilderAnonymous<'facet, T, P, E>
    where
        S: ConfigSource + 'static,
        P: 'static,
    {
        ConfigBuilderAnonymous(self.0.source(source_type_token, source))
    }

    pub fn source_named<S>(
        self,
        name: impl Into<String>,
        source_type_token: SourceRegistration<S, P, E>,
        source: S,
    ) -> ConfigBuilderNamed<'facet, T, P, E>
    where
        S: ConfigSource + 'static,
        P: 'static,
    {
        ConfigBuilderNamed(self.0.source_named(name, source_type_token, source))
    }
}

impl<'facet, T, P, E> ConfigBuilderAnonymous<'facet, T, P, E>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'facet, P, E>,
    P: SourceFieldPath,
{
    pub fn source<S>(
        self,
        source_type_token: SourceRegistration<S, P, E>,
        source: S,
    ) -> ConfigBuilderAnonymous<'facet, T, P, E>
    where
        S: ConfigSource + 'static,
        P: 'static,
    {
        ConfigBuilderAnonymous(self.0.source(source_type_token, source))
    }
}

impl<'facet, T, P, E> ConfigBuilderNamed<'facet, T, P, E>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'facet, P, E>,
    P: SourceFieldPath,
{
    pub fn source_named<S>(
        self,
        name: impl Into<String>,
        source_type_token: SourceRegistration<S, P, E>,
        source: S,
    ) -> ConfigBuilderNamed<'facet, T, P, E>
    where
        S: ConfigSource + 'static,
        P: 'static,
    {
        ConfigBuilderNamed(self.0.source_named(name, source_type_token, source))
    }
}

impl<T, P, E> ConfigBuilderInner<'static, T, P, E>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'static, P, E>,
    P: SourceFieldPath,
{
    pub fn load(self) -> Result<T, ConfigLoadError<E>> {
        let loaded = self.load_with_meta()?;
        Ok(loaded.config)
    }

    pub fn load_with_meta(self) -> Result<LoadedConfig<'static, T, P, E>, ConfigLoadError<E>> {
        load_config_from_sources(self.sources)
    }
}

impl<T, P, E> ConfigBuilderAnonymous<'static, T, P, E>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'static, P, E>,
    P: SourceFieldPath,
{
    pub fn load(self) -> Result<T, ConfigLoadError<E>> {
        self.0.load()
    }

    pub fn load_with_meta(self) -> Result<LoadedConfig<'static, T, P, E>, ConfigLoadError<E>> {
        self.0.load_with_meta()
    }
}

impl<T, P, E> ConfigBuilderNamed<'static, T, P, E>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'static, P, E>,
    P: SourceFieldPath,
{
    pub fn load(self) -> Result<T, ConfigLoadError<E>> {
        self.0.load()
    }

    pub fn load_with_meta(self) -> Result<LoadedConfig<'static, T, P, E>, ConfigLoadError<E>> {
        self.0.load_with_meta()
    }
}

pub struct LoadedConfig<'facet, T, P: SourceFieldPath, E>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'facet, P, E>,
    P: SourceFieldPath,
{
    config: T,
    sources: Vec<(ConfigPath, SourceResolution<P>)>,
    source_index: BTreeMap<ConfigPath, usize>,
    _error_type: PhantomData<&'facet E>,
}

impl<'facet, T, P, E> LoadedConfig<'facet, T, P, E>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'facet, P, E>,
    P: SourceFieldPath,
{
    pub(crate) fn new(config: T, sources: Vec<(ConfigPath, SourceResolution<P>)>) -> Self {
        let source_index = sources
            .iter()
            .enumerate()
            .map(|(index, (path, _))| (path.clone(), index))
            .collect();
        Self {
            config,
            sources,
            source_index,
            _error_type: PhantomData,
        }
    }

    pub fn config(&self) -> &T {
        &self.config
    }
    pub fn into_config(self) -> T {
        self.config
    }
    // r[impl provenance.unset]
    pub fn source(&self, path: &ConfigPath) -> Option<&SourceResolution<P>> {
        let index = *self.source_index.get(path)?;
        let (_, source_resolution) = self.sources.get(index)?;
        Some(source_resolution)
    }
    pub fn shadow_chain(&self, path: &ConfigPath) -> Option<&[ConfigValueSource<P>]> {
        let index = self.source_index.get(path).copied()?;
        let (_, source_resolution) = self.sources.get(index)?;
        Some(source_resolution.shadowed())
    }
    // r[impl provenance.list]
    pub fn sources(&self) -> &[(ConfigPath, SourceResolution<P>)] {
        self.sources.as_slice()
    }
}

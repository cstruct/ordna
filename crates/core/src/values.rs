//! A source of values the app provides itself, such as from command line flags.

use std::convert::Infallible;

use facet_value::Value;

use crate::{
    ConfigLayer, ConfigPath, ConfigSource, ConfigSourceSettings, FieldCandidate, Schema,
    SourceValueDetail,
};

// r[impl values.source]
/// Values for config paths, given by the app, such as from its command line flags:
///
/// ```ignore
/// let mut flags = ValuesSource::new();
/// if let Some(level) = args.log_level {
///     flags.insert(ConfigPath::parse("telemetry.logs.level")?, level);
/// }
/// builder.source_named("flags", registration, flags)
/// ```
///
/// Values are strings, parsed into their field's type when the config is decoded, like
/// environment variables. A path matching no field is reported as an unknown path.
#[derive(Debug, Clone, Default)]
pub struct ValuesSource {
    values: Vec<(ConfigPath, String)>,
    layer: ConfigLayer,
}

impl ValuesSource {
    /// An empty, public source.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the value for `path`. A later value for the same path replaces it.
    pub fn insert(&mut self, path: ConfigPath, value: impl Into<String>) -> &mut Self {
        self.values.retain(|(existing, _)| *existing != path);
        self.values.push((path, value.into()));
        self
    }

    // r[impl values.layer]
    /// Makes this a secret source, for values of secret fields, such as ones read from
    /// files named on the command line.
    pub fn secrets(mut self) -> Self {
        self.layer = ConfigLayer::Secret;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl FromIterator<(ConfigPath, String)> for ValuesSource {
    fn from_iter<I: IntoIterator<Item = (ConfigPath, String)>>(values: I) -> Self {
        let mut source = Self::new();
        for (path, value) in values {
            source.insert(path, value);
        }
        source
    }
}

impl ConfigSource for ValuesSource {
    type Error = Infallible;
    type Detail = ValueDetail;

    // r[impl values.unknown]
    /// Never allows unknown paths, so the engine reports a value for a path that matches no
    /// field, such as a typo in the app's flag handling.
    fn settings(&self) -> ConfigSourceSettings {
        let builder = ConfigSourceSettings::builder();
        match self.layer {
            ConfigLayer::Public => builder.public(),
            ConfigLayer::Secret => builder.secrets(),
        }
        .build()
    }

    // r[impl values.strings]
    fn load(
        &self,
        _schema: &'static Schema,
    ) -> Result<Vec<FieldCandidate<ValueDetail>>, Infallible> {
        Ok(self
            .values
            .iter()
            .map(|(path, value)| {
                FieldCandidate::new(path.clone(), Value::from(value.as_str()), ValueDetail)
            })
            .collect())
    }
}

// r[impl values.detail]
/// A value given by the app; name the source to tell where values came from, such as
/// `flags`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueDetail;

impl SourceValueDetail for ValueDetail {
    fn label(&self) -> String {
        "a value given by the app".to_string()
    }
}

#[cfg(test)]
mod tests {
    use facet::Facet;
    use facet_value::from_value;

    use super::ValuesSource;
    use crate::{ConfigLayer, ConfigPath, ConfigSource, Schema, SourceValueDetail};

    #[derive(Facet)]
    struct App {
        port: u16,
    }

    static SCHEMA: Schema = Schema::new::<App>();

    fn path(path: &str) -> ConfigPath {
        ConfigPath::parse(path).unwrap()
    }

    // r[verify values.source]
    // r[verify values.strings]
    // r[verify values.detail]
    #[test]
    fn provides_the_values_given() {
        let mut source = ValuesSource::new();
        assert!(source.is_empty());
        source
            .insert(path("port"), "80")
            .insert(path("port"), "8080");
        let candidates = source.load(&SCHEMA).unwrap();
        let [candidate] = candidates.as_slice() else {
            panic!("expected one value");
        };
        assert_eq!(candidate.path(), &path("port"));
        assert_eq!(candidate.detail().label(), "a value given by the app");
        assert_eq!(
            from_value::<String>(candidate.value().clone()).unwrap(),
            "8080"
        );

        let source = [(path("port"), "1".to_string())]
            .into_iter()
            .collect::<ValuesSource>();
        assert_eq!(source.load(&SCHEMA).unwrap().len(), 1);
    }

    // r[verify values.layer]
    #[test]
    fn public_unless_secret() {
        assert_eq!(ValuesSource::new().settings().layer, ConfigLayer::Public);
        assert_eq!(
            ValuesSource::new().secrets().settings().layer,
            ConfigLayer::Secret
        );
        assert!(!ValuesSource::new().settings().allow_unknown_paths);
    }
}

//! Environment variable config source.
//!
//! Each leaf field of the config model maps to one variable, built from the optional
//! prefix and the field's config path segments, joined by the separator and uppercased:
//!
//! | prefix | separator | field             | variable                |
//! |--------|-----------|-------------------|-------------------------|
//! | -      | `__`      | `jwt.ttl_seconds` | `JWT__TTL_SECONDS`      |
//! | `APP`  | `__`      | `jwt.ttl_seconds` | `APP__JWT__TTL_SECONDS` |
//!
//! Variable names are matched case-insensitively. Values are passed on as strings and
//! parsed into the field's type when the config is decoded.
//!
//! With a prefix, variables starting with it that match no field are passed on under the
//! path derived from their name (`APP__JWT__TTL` becomes `jwt.ttl`), so the engine can
//! report them as unknown paths. Without a prefix such variables are ignored, since there's
//! no telling which variables are meant for the config.
//!
//! The environment is a public source: secret fields can't be loaded from it.

use std::{collections::BTreeMap, ffi::OsString, fmt};

use facet_value::Value;
use ordna_core::{
    ConfigPath, ConfigPathSegment, ConfigSource, ConfigSourceSettings, FieldCandidate, Schema,
    SourceValueDetail,
};

// r[impl env.separator]
const DEFAULT_SEPARATOR: &str = "__";

/// Loads config values from environment variables.
#[derive(Debug, Clone)]
pub struct EnvSource {
    prefix: Option<String>,
    separator: String,
    allow_unknown_paths: bool,
    /// Explicit variables to read instead of the process environment.
    vars: Option<Vec<(OsString, OsString)>>,
}

impl Default for EnvSource {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvSource {
    /// A source reading the process environment without a prefix.
    pub fn new() -> Self {
        Self {
            prefix: None,
            separator: DEFAULT_SEPARATOR.to_string(),
            allow_unknown_paths: false,
            vars: None,
        }
    }

    /// A source reading the process environment, only considering variables starting with
    /// `prefix` followed by the separator.
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self::new().prefix(prefix)
    }

    /// A source reading `vars` instead of the process environment.
    pub fn from_vars<K, V>(vars: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<OsString>,
        V: Into<OsString>,
    {
        Self {
            vars: Some(
                vars.into_iter()
                    .map(|(key, value)| (key.into(), value.into()))
                    .collect(),
            ),
            ..Self::new()
        }
    }

    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    // r[impl env.separator]
    /// Sets the separator between the prefix and path segments. Defaults to `__`.
    pub fn separator(mut self, separator: impl Into<String>) -> Self {
        let separator = separator.into();
        assert!(!separator.is_empty(), "the separator must not be empty");
        self.separator = separator;
        self
    }

    /// Whether prefixed variables that match no field are allowed. Defaults to `false`.
    pub fn allow_unknown_paths(mut self, allow: bool) -> Self {
        self.allow_unknown_paths = allow;
        self
    }

    /// The (uppercased) variable name prefix, including the trailing separator.
    fn name_prefix(&self) -> Option<String> {
        self.prefix
            .as_ref()
            .map(|prefix| format!("{}{}", prefix, self.separator).to_uppercase())
    }

    fn vars(&self) -> Vec<(OsString, OsString)> {
        match &self.vars {
            Some(vars) => vars.clone(),
            None => std::env::vars_os().collect(),
        }
    }
}

impl ConfigSource for EnvSource {
    type Error = EnvSourceError;
    type Detail = EnvVarDetail;

    // r[impl env.public]
    /// Always public: the environment is too easily leaked (process listings, crash
    /// reports, child processes) to hold secrets.
    fn settings(&self) -> ConfigSourceSettings {
        ConfigSourceSettings::builder()
            .public()
            .allow_unknown_paths(self.allow_unknown_paths)
            .build()
    }

    fn load(
        &self,
        schema: &'static Schema,
    ) -> Result<Vec<FieldCandidate<EnvVarDetail>>, Self::Error> {
        let name_prefix = self.name_prefix();
        let fields = schema
            .fields()
            .into_iter()
            .map(|path| {
                let segments = path
                    .segments()
                    .iter()
                    .map(|segment| segment.as_str())
                    .collect::<Vec<_>>()
                    .join(&self.separator);
                // r[impl env.naming]
                let name = format!("{}{}", name_prefix.as_deref().unwrap_or(""), segments);
                (name.to_uppercase(), path)
            })
            .collect::<BTreeMap<_, _>>();

        let mut candidates = Vec::new();
        for (name, value) in self.vars() {
            // Variables with non-unicode names can't match any field.
            let Some(name) = name.to_str() else {
                continue;
            };
            // r[impl env.case-insensitive]
            let upper = name.to_uppercase();
            let detail = EnvVarDetail {
                name: name.to_string(),
            };
            // r[impl env.values]
            // r[impl env.not-unicode]
            if let Some(path) = fields.get(&upper) {
                let value = value
                    .into_string()
                    .map_err(|_| EnvSourceError::NotUnicode {
                        name: name.to_string(),
                    })?;
                candidates.push(FieldCandidate::new(
                    path.clone(),
                    Value::from(value),
                    detail,
                ));
                continue;
            }

            // r[impl env.unprefixed]
            let Some(prefix) = &name_prefix else {
                continue;
            };
            let Some(rest) = upper.strip_prefix(prefix.as_str()) else {
                continue;
            };
            // r[impl env.prefixed-unknown]
            // r[impl env.invalid-name]
            match self.path_from_name(rest) {
                // The engine reports or drops unknown paths based on the source settings.
                Some(path) => candidates.push(FieldCandidate::new(
                    path,
                    Value::from(value.to_string_lossy().into_owned()),
                    detail,
                )),
                None if self.allow_unknown_paths => {}
                None => {
                    return Err(EnvSourceError::InvalidName {
                        name: name.to_string(),
                    });
                }
            }
        }
        Ok(candidates)
    }
}

impl EnvSource {
    /// Derives a config path from a variable name with the prefix stripped, such as
    /// `JWT__TTL` becoming `jwt.ttl`.
    fn path_from_name(&self, name: &str) -> Option<ConfigPath> {
        let segments = name
            .split(self.separator.as_str())
            .map(|segment| ConfigPathSegment::parse(segment.to_lowercase()).ok())
            .collect::<Option<Vec<_>>>()?;
        Some(ConfigPath::new(segments))
    }
}

// r[impl env.detail]
/// Describes which environment variable a value came from.
#[derive(Debug, Clone)]
pub struct EnvVarDetail {
    pub name: String,
}

impl SourceValueDetail for EnvVarDetail {
    fn label(&self) -> String {
        format!("environment variable `{}`", self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvSourceError {
    /// A variable matching a field has a value that isn't valid unicode.
    NotUnicode { name: String },
    /// A variable with the source's prefix matches no field, and its name can't be turned
    /// into a config path to report it as an unknown path.
    InvalidName { name: String },
}

impl fmt::Display for EnvSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotUnicode { name } => {
                write!(f, "environment variable `{name}` is not valid unicode")
            }
            Self::InvalidName { name } => write!(
                f,
                "environment variable `{name}` matches no config field and is not a valid config path"
            ),
        }
    }
}

impl std::error::Error for EnvSourceError {}

#[cfg(test)]
mod tests {
    use facet::Facet;
    use facet_value::from_value;
    use ordna_core::{ConfigSource, Schema, SourceValueDetail};

    use super::{EnvSource, EnvSourceError};

    #[derive(Facet, Debug, PartialEq)]
    struct Jwt {
        ttl_seconds: u32,
        issuer: Option<String>,
    }

    #[derive(Facet, Debug, PartialEq)]
    struct App {
        name: String,
        debug: bool,
        jwt: Jwt,
    }

    static SCHEMA: Schema = Schema::new::<App>();

    fn load(source: &EnvSource) -> Result<Vec<(String, String)>, EnvSourceError> {
        let mut values = source
            .load(&SCHEMA)?
            .into_iter()
            .map(|candidate| {
                (
                    candidate.detail().label(),
                    from_value::<String>(candidate.value().clone()).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        values.sort();
        Ok(values)
    }

    // r[verify env.naming]
    // r[verify env.case-insensitive]
    // r[verify env.unprefixed]
    // r[verify env.values]
    // r[verify env.detail]
    #[test]
    fn maps_variables_to_fields() {
        let source = EnvSource::from_vars([
            ("NAME", "app"),
            ("JWT__TTL_SECONDS", "60"),
            ("jwt__issuer", "me"),
            ("PATH", "/usr/bin"),
        ]);
        assert_eq!(
            load(&source).unwrap(),
            vec![
                (
                    "environment variable `JWT__TTL_SECONDS`".into(),
                    "60".into()
                ),
                ("environment variable `NAME`".into(), "app".into()),
                ("environment variable `jwt__issuer`".into(), "me".into()),
            ]
        );
    }

    // r[verify env.naming]
    // r[verify env.separator]
    #[test]
    fn prefix_and_separator() {
        let source = EnvSource::from_vars([
            ("APP_NAME", "app"),
            ("APP_JWT_TTL_SECONDS", "60"),
            ("NAME", "ignored"),
        ])
        .prefix("app")
        .separator("_");
        assert_eq!(
            load(&source).unwrap(),
            vec![
                (
                    "environment variable `APP_JWT_TTL_SECONDS`".into(),
                    "60".into()
                ),
                ("environment variable `APP_NAME`".into(), "app".into()),
            ]
        );
    }

    // r[verify env.prefixed-unknown]
    #[test]
    fn unknown_prefixed_variables_get_dynamic_paths() {
        let source =
            EnvSource::from_vars([("APP__NAME", "app"), ("APP__JWT__TLL", "typo")]).prefix("APP");
        let mut paths = source
            .load(&SCHEMA)
            .unwrap()
            .iter()
            .map(|candidate| candidate.path().to_string())
            .collect::<Vec<_>>();
        paths.sort();
        assert_eq!(paths, vec!["jwt.tll", "name"]);
    }

    // r[verify env.invalid-name]
    #[test]
    fn unrepresentable_prefixed_variables() {
        let vars = [("APP__NAME", "app"), ("APP__1NAME", "bad")];
        let source = EnvSource::from_vars(vars).prefix("APP");
        assert_eq!(
            load(&source),
            Err(EnvSourceError::InvalidName {
                name: "APP__1NAME".into()
            })
        );

        let source = EnvSource::from_vars(vars)
            .prefix("APP")
            .allow_unknown_paths(true);
        assert_eq!(load(&source).unwrap().len(), 1);
    }

    // r[verify env.not-unicode]
    #[cfg(unix)]
    #[test]
    fn non_unicode_values_fail() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let source =
            EnvSource::from_vars([(OsString::from("NAME"), OsString::from_vec(vec![0xff, 0xfe]))]);
        assert_eq!(
            source.load(&SCHEMA).map(|_| ()),
            Err(EnvSourceError::NotUnicode {
                name: "NAME".into()
            })
        );
    }

    // r[verify env.public]
    #[test]
    fn is_always_public() {
        let source = EnvSource::new().allow_unknown_paths(true);
        assert_eq!(source.settings().layer, ordna_core::ConfigLayer::Public);
    }

    // r[verify env.values]
    // r[verify load.decode.strings]
    #[test]
    fn values_decode_into_field_types() {
        let source = EnvSource::from_vars([
            ("NAME", "app"),
            ("DEBUG", "true"),
            ("JWT__TTL_SECONDS", "60"),
        ]);
        let candidates = source.load(&SCHEMA).unwrap();
        let mut root = facet_value::VObject::new();
        let mut jwt = facet_value::VObject::new();
        for candidate in candidates {
            match candidate.path().segments() {
                [leaf] => root.insert(leaf.as_str(), candidate.value().clone()),
                [_, leaf] => jwt.insert(leaf.as_str(), candidate.value().clone()),
                _ => unreachable!(),
            };
        }
        root.insert("jwt", jwt);
        assert_eq!(
            from_value::<App>(root.into()).unwrap(),
            App {
                name: "app".into(),
                debug: true,
                jwt: Jwt {
                    ttl_seconds: 60,
                    issuer: None,
                },
            }
        );
    }
}

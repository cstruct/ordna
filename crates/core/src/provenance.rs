use crate::{ConfigPath, source::SourceFieldPath};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoDerivedKind {}

/// Where a config value came from, optionally derived from another source.
///
/// - **P** describes the direct sources that may appear in the provenance chain. Each
///   variant should carry enough data to explain where a direct value came from, in a way
///   that is useful for diagnostics, debugging and shadow-chain inspection. Sources'
///   value details are mapped into `P` when they are registered, see
///   [`crate::SourceRegistration`].
/// - **D** describes the derivation steps that may appear in the provenance chain, such as
///   resolved file paths, lookup keys or helper names.
///
/// Without derived values:
///
/// ```rust
/// use std::path::PathBuf;
///
/// use ::core::{ConfigPath, ConfigValueSource, SourceFieldPath};
///
/// enum Origin {
///     File { file: PathBuf, path: ConfigPath },
///     Env { var: String, path: ConfigPath },
/// }
///
/// impl SourceFieldPath for Origin {
///     fn field_path(&self) -> &ConfigPath {
///         match self {
///             Origin::File { path, .. } | Origin::Env { path, .. } => path,
///         }
///     }
/// }
///
/// let source = ConfigValueSource::direct(Origin::File {
///     file: PathBuf::from("config.styx"),
///     path: ConfigPath::parse("jwt.ttl_seconds").unwrap(),
/// });
/// assert_eq!(source.field_path().to_string(), "jwt.ttl_seconds");
/// ```
///
/// With derived values, `D` names the derivation steps:
///
/// ```rust
/// # use std::path::PathBuf;
/// # use ::core::{ConfigPath, ConfigValueSource, SourceFieldPath};
/// # struct Origin { file: PathBuf, path: ConfigPath }
/// # impl SourceFieldPath for Origin {
/// #     fn field_path(&self) -> &ConfigPath {
/// #         &self.path
/// #     }
/// # }
/// enum Derivation {
///     PrivateKeyFile { resolved: PathBuf },
/// }
///
/// let configured_by = ConfigValueSource::Direct(Origin {
///     file: PathBuf::from("config.styx"),
///     path: ConfigPath::parse("jwt.private_key_path").unwrap(),
/// });
/// let source = ConfigValueSource::derived(
///     Derivation::PrivateKeyFile {
///         resolved: PathBuf::from("/etc/app/private_key.pem"),
///     },
///     configured_by,
/// );
/// assert_eq!(source.field_path().to_string(), "jwt.private_key_path");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValueSource<P: SourceFieldPath, D = NoDerivedKind> {
    Direct(P),
    Derived {
        kind: D,
        configured_by: Box<ConfigValueSource<P, D>>,
    },
}

impl<P: SourceFieldPath> ConfigValueSource<P, NoDerivedKind> {
    pub fn direct(p: P) -> Self {
        Self::Direct(p)
    }
}

impl<P: SourceFieldPath, D> ConfigValueSource<P, D> {
    pub fn derived(kind: D, configured_by: ConfigValueSource<P, D>) -> Self {
        Self::Derived {
            kind,
            configured_by: Box::new(configured_by),
        }
    }
}

// r[impl provenance.derived]
impl<P: SourceFieldPath, D> SourceFieldPath for ConfigValueSource<P, D> {
    fn field_path(&self) -> &ConfigPath {
        match self {
            ConfigValueSource::Direct(p) => p.field_path(),
            ConfigValueSource::Derived { configured_by, .. } => configured_by.field_path(),
        }
    }

    fn source_kind(&self) -> &'static str {
        match self {
            ConfigValueSource::Direct(p) => p.source_kind(),
            ConfigValueSource::Derived { configured_by, .. } => configured_by.source_kind(),
        }
    }
}

/// Where the value of a field came from: the winning source, and the sources it shadowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceResolution<P: SourceFieldPath> {
    winner: ConfigValueSource<P>,
    shadowed: Vec<ConfigValueSource<P>>,
}

impl<P: SourceFieldPath> SourceResolution<P> {
    pub(crate) fn new(winner: ConfigValueSource<P>, shadowed: Vec<ConfigValueSource<P>>) -> Self {
        Self { winner, shadowed }
    }

    /// The source whose value was used.
    pub fn winner(&self) -> &ConfigValueSource<P> {
        &self.winner
    }
    /// The sources whose values were shadowed by the winner, most recent first.
    pub fn shadowed(&self) -> &[ConfigValueSource<P>] {
        self.shadowed.as_slice()
    }
}

impl<P: SourceFieldPath> SourceFieldPath for SourceResolution<P> {
    fn field_path(&self) -> &ConfigPath {
        self.winner.field_path()
    }

    fn source_kind(&self) -> &'static str {
        self.winner.source_kind()
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigValueSource;
    use crate::{ConfigPath, SourceFieldPath};

    struct Origin(ConfigPath);

    impl SourceFieldPath for Origin {
        fn field_path(&self) -> &ConfigPath {
            &self.0
        }
        fn source_kind(&self) -> &'static str {
            "file"
        }
    }

    // r[verify provenance.derived]
    #[test]
    fn derived_sources_follow_their_configuring_source() {
        let path = ConfigPath::parse("jwt.private_key_path").unwrap();
        let source = ConfigValueSource::derived(
            "read key file",
            ConfigValueSource::derived(
                "resolve path",
                ConfigValueSource::Direct(Origin(path.clone())),
            ),
        );
        assert_eq!(source.field_path(), &path);
        assert_eq!(source.source_kind(), "file");
    }
}

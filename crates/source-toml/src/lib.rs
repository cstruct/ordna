//! [TOML](https://toml.io) config files.
//!
//! ```toml
//! name = "app"
//!
//! [jwt]
//! ttl_seconds = 60
//! ```

use std::path::PathBuf;

use facet_value::Value;
use ordna_source_file::{FileFormat, FileSource};

pub use facet_toml::DeserializeError;

/// The TOML [`FileFormat`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Toml;

// r[impl format.toml]
impl FileFormat for Toml {
    type Error = DeserializeError;

    fn name(&self) -> &'static str {
        "toml"
    }

    fn parse(&self, contents: &str) -> Result<Value, Self::Error> {
        facet_toml::from_str::<Value>(contents)
    }
}

/// A source reading a TOML file.
pub type TomlSource = FileSource<Toml>;

/// A source reading the TOML file at `path`, which must exist.
pub fn toml_file(path: impl Into<PathBuf>) -> TomlSource {
    FileSource::new(path, Toml)
}

#[cfg(test)]
mod tests {
    use facet::Facet;
    use facet_value::from_value;
    use ordna_core::{ConfigSource, Schema};
    use ordna_source_file::FileSourceError;

    use super::toml_file;

    #[derive(Facet)]
    struct Jwt {
        ttl_seconds: u32,
        issuer: Option<String>,
    }

    #[derive(Facet)]
    struct App {
        name: String,
        tags: Vec<String>,
        jwt: Jwt,
    }

    static SCHEMA: Schema = Schema::new::<App>();

    fn write(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ordna-source-toml-{name}-{}.toml",
            std::process::id()
        ));
        std::fs::write(&path, contents).unwrap();
        path
    }

    // r[verify format.toml]
    #[test]
    fn loads_toml_files() {
        let path = write(
            "load",
            r#"name = "app"
tags = ["a", "b"]

[jwt]
ttl_seconds = 60
issuer = "me there"
"#,
        );
        let mut values = toml_file(&path)
            .load(&SCHEMA)
            .unwrap()
            .into_iter()
            .map(|candidate| (candidate.path().to_string(), candidate.value().clone()))
            .collect::<Vec<_>>();
        values.sort_by(|a, b| a.0.cmp(&b.0));

        let paths = values
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["jwt.issuer", "jwt.ttl_seconds", "name", "tags"]);
        assert_eq!(
            from_value::<String>(values[0].1.clone()).unwrap(),
            "me there"
        );
        assert_eq!(from_value::<u32>(values[1].1.clone()).unwrap(), 60);
        assert_eq!(from_value::<String>(values[2].1.clone()).unwrap(), "app");
        assert_eq!(
            from_value::<Vec<String>>(values[3].1.clone()).unwrap(),
            vec!["a", "b"]
        );
    }

    // r[verify file.parse-error]
    #[test]
    fn reports_parse_errors() {
        let path = write(
            "invalid", r#"[jwt
"#,
        );
        assert!(matches!(
            toml_file(&path).load(&SCHEMA),
            Err(FileSourceError::Parse { format: "toml", .. })
        ));
    }
}

//! [Styx](https://github.com/bearcove/styx) config files.
//!
//! ```styx
//! name app
//! jwt {
//!   ttl_seconds 60
//! }
//! ```
//!
//! Styx scalars are untyped, so every scalar is passed on as a string and parsed into the
//! field's type when the config is decoded.

use std::path::PathBuf;

use facet_value::Value;
use ordna_source_file::{FileFormat, FileSource};

pub use facet_styx::DeserializeError;

/// The Styx [`FileFormat`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Styx;

// r[impl format.styx]
impl FileFormat for Styx {
    type Error = DeserializeError;

    fn name(&self) -> &'static str {
        "styx"
    }

    fn parse(&self, contents: &str) -> Result<Value, Self::Error> {
        facet_styx::from_str::<Value>(contents)
    }
}

/// A source reading a Styx file.
pub type StyxSource = FileSource<Styx>;

/// A source reading the Styx file at `path`, which must exist.
pub fn styx_file(path: impl Into<PathBuf>) -> StyxSource {
    FileSource::new(path, Styx)
}

#[cfg(test)]
mod tests {
    use facet::Facet;
    use facet_value::from_value;
    use ordna_core::{ConfigSource, Schema};
    use ordna_source_file::FileSourceError;

    use super::styx_file;

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
        let path = std::env::temp_dir().join(format!("ordna-source-styx-{name}.styx"));
        std::fs::write(&path, contents).unwrap();
        path
    }

    // r[verify format.styx]
    #[test]
    fn loads_styx_files() {
        let path = write(
            "load",
            "name app\ntags (a b)\njwt {\n  ttl_seconds 60\n  issuer \"me there\"\n}\n",
        );
        let mut values = styx_file(&path)
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
        assert_eq!(
            from_value::<Vec<String>>(values[3].1.clone()).unwrap(),
            vec!["a", "b"]
        );
    }

    // r[verify file.parse-error]
    #[test]
    fn reports_parse_errors() {
        let path = write("invalid", "jwt {\n");
        assert!(matches!(
            styx_file(&path).load(&SCHEMA),
            Err(FileSourceError::Parse { format: "styx", .. })
        ));
    }
}

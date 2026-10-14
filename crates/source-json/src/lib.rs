//! [JSON](https://www.json.org) config files.
//!
//! ```json
//! {
//!   "name": "app",
//!   "jwt": { "ttl_seconds": 60 }
//! }
//! ```

use std::path::PathBuf;

use facet_value::Value;
use source_file::{FileFormat, FileSource};

pub use facet_json::DeserializeError;

/// The JSON [`FileFormat`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Json;

// r[impl format.json]
impl FileFormat for Json {
    type Error = DeserializeError;

    fn name(&self) -> &'static str {
        "json"
    }

    fn parse(&self, contents: &str) -> Result<Value, Self::Error> {
        facet_json::from_str::<Value>(contents)
    }
}

/// A source reading a JSON file.
pub type JsonSource = FileSource<Json>;

/// A source reading the JSON file at `path`, which must exist.
pub fn json_file(path: impl Into<PathBuf>) -> JsonSource {
    FileSource::new(path, Json)
}

#[cfg(test)]
mod tests {
    use facet::Facet;
    use facet_value::from_value;
    use ordna_core::{ConfigSource, Schema};
    use source_file::FileSourceError;

    use super::json_file;

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
            "ordna-source-json-{name}-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, contents).unwrap();
        path
    }

    // r[verify format.json]
    #[test]
    fn loads_json_files() {
        let path = write(
            "load",
            r#"{
  "name": "app",
  "tags": ["a", "b"],
  "jwt": {
    "ttl_seconds": 60,
    "issuer": "me there"
  }
}
"#,
        );
        let mut values = json_file(&path)
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
        let path = write("invalid", r#"{ "jwt": "#);
        assert!(matches!(
            json_file(&path).load(&SCHEMA),
            Err(FileSourceError::Parse { format: "json", .. })
        ));
    }
}

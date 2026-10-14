//! Generic file config source.
//!
//! A [`FileSource`] reads a file and parses it with a [`FileFormat`] into a dynamic
//! [`Value`]. The root must be an object; its keys are walked, guided by the schema, into
//! config paths:
//!
//! - a key whose path is a schema field becomes a candidate for that field, whatever its
//!   value (so fields holding maps or lists receive the whole object or array),
//! - an object whose path leads towards schema fields is walked further,
//! - anything else becomes a candidate under its own (unknown) path, which the engine
//!   reports or drops based on the source settings.
//!
//! A file holds a single layer: by default it's public and may only set public fields; a
//! file marked with [`FileSource::secrets`] may only set secret fields. Keep secrets in a
//! separate file from the rest of the config.

use std::{
    fmt, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use facet_value::{VObject, Value};
use ordna_core::{
    ConfigLayer, ConfigPath, ConfigPathSegment, ConfigSource, ConfigSourceSettings, FieldCandidate,
    Schema, SourceValueDetail,
};

// r[impl format.mixed]
/// A file format that can be parsed into a dynamic [`Value`].
pub trait FileFormat: Send + Sync + 'static {
    type Error: fmt::Debug + fmt::Display + Send + Sync + 'static;

    /// The name of the format, such as `styx`.
    fn name(&self) -> &'static str;

    fn parse(&self, contents: &str) -> Result<Value, Self::Error>;
}

/// Loads config values from a file in format `F`.
#[derive(Debug, Clone)]
pub struct FileSource<F> {
    path: PathBuf,
    format: F,
    required: bool,
    settings: ConfigSourceSettings,
}

impl<F: FileFormat> FileSource<F> {
    /// A source reading the file at `path`, which must exist.
    pub fn new(path: impl Into<PathBuf>, format: F) -> Self {
        Self {
            path: path.into(),
            format,
            required: true,
            settings: ConfigSourceSettings::builder().build(),
        }
    }

    /// Makes a missing file provide no values instead of failing.
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    // r[impl file.layer]
    /// Makes this a secrets file, which may only set secret fields.
    pub fn secrets(mut self) -> Self {
        self.settings.layer = ConfigLayer::Secret;
        self
    }

    pub fn settings(mut self, settings: ConfigSourceSettings) -> Self {
        self.settings = settings;
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    // r[impl file.missing]
    fn read(&self) -> Result<Option<String>, FileSourceError<F::Error>> {
        match std::fs::read_to_string(&self.path) {
            Ok(contents) => Ok(Some(contents)),
            Err(error) if error.kind() == io::ErrorKind::NotFound && !self.required => Ok(None),
            Err(error) => Err(FileSourceError::Io {
                path: self.path.clone(),
                error: Arc::new(error),
            }),
        }
    }
}

impl<F: FileFormat> ConfigSource for FileSource<F> {
    type Error = FileSourceError<F::Error>;
    type Detail = FileValueDetail;

    fn settings(&self) -> ConfigSourceSettings {
        self.settings.clone()
    }

    fn load(
        &self,
        schema: &'static Schema,
    ) -> Result<Vec<FieldCandidate<FileValueDetail>>, Self::Error> {
        let Some(contents) = self.read()? else {
            return Ok(Vec::new());
        };
        // r[impl file.parse-error]
        let root = self
            .format
            .parse(&contents)
            .map_err(|error| FileSourceError::Parse {
                path: self.path.clone(),
                format: self.format.name(),
                error,
            })?;
        // r[impl file.root-object]
        let Some(root) = root.as_object() else {
            return Err(FileSourceError::NotAnObject {
                path: self.path.clone(),
            });
        };

        let mut walker = Walker {
            source: self,
            schema,
            candidates: Vec::new(),
        };
        walker.walk(root, &mut Vec::new())?;
        Ok(walker.candidates)
    }
}

struct Walker<'a, F> {
    source: &'a FileSource<F>,
    schema: &'static Schema,
    candidates: Vec<FieldCandidate<FileValueDetail>>,
}

impl<F: FileFormat> Walker<'_, F> {
    // r[impl file.walk]
    fn walk(
        &mut self,
        object: &VObject,
        segments: &mut Vec<ConfigPathSegment>,
    ) -> Result<(), FileSourceError<F::Error>> {
        for (key, value) in object.iter() {
            // r[impl file.invalid-keys]
            let segment = match ConfigPathSegment::parse(key.as_str().to_string()) {
                Ok(segment) => segment,
                Err(_) if self.source.settings.allow_unknown_paths => continue,
                Err(_) => {
                    let mut key_path = segments
                        .iter()
                        .map(|segment| segment.as_str())
                        .collect::<Vec<_>>();
                    key_path.push(key.as_str());
                    return Err(FileSourceError::InvalidKey {
                        path: self.source.path.clone(),
                        key: key_path.join("."),
                    });
                }
            };
            segments.push(segment);
            let path = ConfigPath::new(segments.clone());
            match value.as_object() {
                Some(object)
                    if self.schema.layer(&path).is_none()
                        && self.schema.leads_to_fields(segments) =>
                {
                    self.walk(object, segments)?;
                }
                // r[impl file.unknown-keys]
                _ => self.candidates.push(FieldCandidate::new(
                    path.clone(),
                    value.clone(),
                    FileValueDetail {
                        file: self.source.path.clone(),
                        key: path.to_string(),
                    },
                )),
            }
            segments.pop();
        }
        Ok(())
    }
}

// r[impl file.detail]
/// Describes which file and key a value came from.
#[derive(Debug, Clone)]
pub struct FileValueDetail {
    pub file: PathBuf,
    pub key: String,
}

impl SourceValueDetail for FileValueDetail {
    fn label(&self) -> String {
        format!("`{}` in `{}`", self.key, self.file.display())
    }
}

#[derive(Debug, Clone)]
pub enum FileSourceError<E> {
    /// The file could not be read.
    Io {
        path: PathBuf,
        error: Arc<io::Error>,
    },
    /// The file could not be parsed.
    Parse {
        path: PathBuf,
        format: &'static str,
        error: E,
    },
    /// The root of the file is not an object.
    NotAnObject { path: PathBuf },
    /// A key is not a valid config path segment.
    InvalidKey { path: PathBuf, key: String },
}

impl<E: fmt::Display> fmt::Display for FileSourceError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "failed to read `{}`: {error}", path.display()),
            Self::Parse {
                path,
                format,
                error,
            } => write!(
                f,
                "failed to parse `{}` as {format}: {error}",
                path.display()
            ),
            Self::NotAnObject { path } => {
                write!(f, "the root of `{}` is not an object", path.display())
            }
            Self::InvalidKey { path, key } => write!(
                f,
                "key `{key}` in `{}` is not a valid config path",
                path.display()
            ),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for FileSourceError<E> {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use facet::Facet;
    use facet_value::{VObject, Value};
    use ordna_core::{ConfigLayer, ConfigSource, ConfigSourceSettings, Schema};

    use super::{FileFormat, FileSource, FileSourceError};

    #[derive(Facet)]
    struct Jwt {
        ttl_seconds: u32,
    }

    #[derive(Facet)]
    struct App {
        name: String,
        tags: Vec<String>,
        jwt: Jwt,
    }

    static SCHEMA: Schema = Schema::new::<App>();

    /// Ignores the file contents and returns a fixed value.
    #[derive(Debug, Clone)]
    struct Fixed(Value);

    impl FileFormat for Fixed {
        type Error = String;
        fn name(&self) -> &'static str {
            "fixed"
        }
        fn parse(&self, _contents: &str) -> Result<Value, String> {
            Ok(self.0.clone())
        }
    }

    fn object(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
        let mut object = VObject::new();
        for (key, value) in entries {
            object.insert(key, value);
        }
        object.into()
    }

    fn temp_file(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("ordna-source-file-{name}"));
        std::fs::write(&path, "").unwrap();
        path
    }

    fn paths(source: &FileSource<Fixed>) -> Vec<String> {
        let mut paths = source
            .load(&SCHEMA)
            .unwrap()
            .iter()
            .map(|candidate| candidate.path().to_string())
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    // r[verify file.walk]
    // r[verify file.unknown-keys]
    #[test]
    fn walks_objects_into_config_paths() {
        let value = object([
            ("name", Value::from("app")),
            (
                "tags",
                Value::from(facet_value::VArray::from_iter([Value::from("a")])),
            ),
            (
                "jwt",
                object([
                    ("ttl_seconds", Value::from("60")),
                    ("tll", Value::from("1")),
                ]),
            ),
            ("extra", object([("nested", Value::from("x"))])),
        ]);
        let source = FileSource::new(temp_file("walk"), Fixed(value));
        assert_eq!(
            paths(&source),
            vec!["extra", "jwt.tll", "jwt.ttl_seconds", "name", "tags"]
        );
    }

    // r[verify file.walk]
    #[test]
    fn walks_into_map_entries() {
        #[derive(Facet)]
        struct WithLabels {
            labels: std::collections::HashMap<String, String>,
        }
        static LABELS: Schema = Schema::new::<WithLabels>();

        let value = object([(
            "labels",
            object([
                ("team", Value::from("storage")),
                ("tier", Value::from("hot")),
            ]),
        )]);
        let source = FileSource::new(temp_file("maps"), Fixed(value));
        let mut paths = source
            .load(&LABELS)
            .unwrap()
            .iter()
            .map(|candidate| candidate.path().to_string())
            .collect::<Vec<_>>();
        paths.sort();
        assert_eq!(paths, vec!["labels.team", "labels.tier"]);
    }

    // r[verify file.missing]
    #[test]
    fn missing_files() {
        let path = std::env::temp_dir().join("ordna-source-file-does-not-exist");
        let source = FileSource::new(&path, Fixed(object([])));
        assert!(matches!(
            source.load(&SCHEMA),
            Err(FileSourceError::Io { .. })
        ));
        assert!(paths(&source.optional()).is_empty());
    }

    // r[verify file.root-object]
    #[test]
    fn root_must_be_an_object() {
        let source = FileSource::new(temp_file("root"), Fixed(Value::from("nope")));
        assert!(matches!(
            source.load(&SCHEMA),
            Err(FileSourceError::NotAnObject { .. })
        ));
    }

    // r[verify file.layer]
    #[test]
    fn files_hold_a_single_layer() {
        let source = FileSource::new(temp_file("layer"), Fixed(object([])));
        assert_eq!(ConfigSource::settings(&source).layer, ConfigLayer::Public);
        assert_eq!(
            ConfigSource::settings(&source.secrets()).layer,
            ConfigLayer::Secret
        );
    }

    // r[verify file.invalid-keys]
    #[test]
    fn invalid_keys() {
        let value = object([("jwt", object([("ttl seconds", Value::from("1"))]))]);
        let source = FileSource::new(temp_file("keys"), Fixed(value));
        let Err(FileSourceError::InvalidKey { key, .. }) = source.load(&SCHEMA) else {
            panic!("expected an invalid key");
        };
        assert_eq!(key, "jwt.ttl seconds");

        let source = source.settings(
            ConfigSourceSettings::builder()
                .allow_unknown_paths(true)
                .build(),
        );
        assert!(paths(&source).is_empty());
    }
}

//! The config model, provenance type and source setup shared by the integration tests.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use ordna_core::{
    ConfigBuilder, ConfigLoadError, ConfigModel, ConfigPath, LoadedConfig, SourceFieldPath,
    SourceRegistration, config_tree,
};
use source_env::{EnvSource, EnvSourceError, EnvVarDetail};
use source_file::{FileFormat, FileSource, FileSourceError, FileValueDetail};
use source_styx::{DeserializeError, styx_file};

config_tree! {
    #[derive(Debug, PartialEq)]
    pub struct App {
        pub name: String,
        pub port: u16,
        pub debug: bool,
        #[config(path = "db")]
        pub database: pub struct Database {
            pub host: String,
            pub port: u16,
            #[config(secret)]
            pub password: String,
        },
        #[config(secret)]
        pub jwt: pub struct Jwt {
            pub signing_key: String,
            pub ttl_seconds: u32,
        },
        pub log: pub struct Log {
            pub level: String,
            pub file: Option<String>,
        },
    }
}

/// Where a config value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Env {
        var: String,
        path: ConfigPath,
    },
    File {
        file: PathBuf,
        key: String,
        path: ConfigPath,
    },
}

impl SourceFieldPath for Origin {
    fn field_path(&self) -> &ConfigPath {
        match self {
            Origin::Env { path, .. } | Origin::File { path, .. } => path,
        }
    }

    fn source_kind(&self) -> &'static str {
        match self {
            Origin::Env { .. } => "env",
            Origin::File { .. } => "file",
        }
    }
}

#[derive(Debug)]
pub enum AppError {
    Env(EnvSourceError),
    /// All facet based formats share facet-format's `DeserializeError`.
    File(FileSourceError<DeserializeError>),
}

pub type Loaded = LoadedConfig<'static, App, Origin, AppError>;
pub type LoadResult = Result<Loaded, ConfigLoadError<AppError>>;

pub fn env_registration() -> SourceRegistration<EnvSource, Origin, AppError> {
    App::register_source(AppError::Env, |path, detail: EnvVarDetail| Origin::Env {
        var: detail.name,
        path,
    })
}

pub fn file_registration<F>() -> SourceRegistration<FileSource<F>, Origin, AppError>
where
    F: FileFormat<Error = DeserializeError>,
{
    App::register_source(AppError::File, |path, detail: FileValueDetail| {
        Origin::File {
            file: detail.file,
            key: detail.key,
            path,
        }
    })
}

/// Loads `App` from, in order of precedence (lowest first): the public `defaults` file,
/// the `secrets` file and `env`.
pub fn load(defaults: &Path, secrets: &Path, env: EnvSource) -> LoadResult {
    ConfigBuilder::new()
        .source_named("defaults", file_registration(), styx_file(defaults))
        .source_named("secrets", file_registration(), styx_file(secrets).secrets())
        .source_named("env", env_registration(), env)
        .load_with_meta()
}

/// A temporary directory for one test, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("ordna-integration-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub const DEFAULTS: &str = "\
name ordna
port 8080
debug false
db {
  host localhost
  port 5432
}
log {
  level info
}
";

pub const SECRETS: &str = "\
db {
  password hunter2
}
jwt {
  signing_key \"s3cr3t key\"
  ttl_seconds 3600
}
";

pub fn path(path: &str) -> ConfigPath {
    ConfigPath::parse(path).unwrap()
}

/// The expected config when loading [`DEFAULTS`] and [`SECRETS`] with `APP__PORT=9090`,
/// `APP__DEBUG=true` and `APP__LOG__LEVEL=debug` set.
pub fn expected_app() -> App {
    App {
        name: "ordna".into(),
        port: 9090,
        debug: true,
        database: Database {
            host: "localhost".into(),
            port: 5432,
            password: "hunter2".into(),
        },
        jwt: Jwt {
            signing_key: "s3cr3t key".into(),
            ttl_seconds: 3600,
        },
        log: Log {
            level: "debug".into(),
            file: None,
        },
    }
}

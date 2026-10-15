//! Loading a config from a file, the environment and values the app gives, such as from
//! its command line flags.

mod common;

use std::convert::Infallible;

use common::{TempDir, path};
use ordna_core::{
    ConfigBuilder, ConfigLoadError, ConfigModel, ConfigPath, ConfigValueSource, LoadedConfig,
    SourceFieldPath, ValueDetail, ValuesSource, config_tree,
};
use ordna_source_env::{EnvSource, EnvSourceError, EnvVarDetail};
use ordna_source_file::{FileSourceError, FileValueDetail};
use ordna_source_yaml::{DeserializeError, yaml_file};

config_tree! {
    #[derive(Debug, PartialEq)]
    pub struct App {
        pub name: String,
        pub port: u16,
        pub debug: bool,
        pub database: pub struct Database {
            pub host: String,
            #[config(secret)]
            pub password: String,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Origin {
    File(ConfigPath),
    Env(ConfigPath),
    Given(ConfigPath),
}

impl SourceFieldPath for Origin {
    fn field_path(&self) -> &ConfigPath {
        match self {
            Origin::File(path) | Origin::Env(path) | Origin::Given(path) => path,
        }
    }
}

#[derive(Debug)]
#[allow(dead_code, reason = "only read through `Debug`")]
enum AppError {
    File(FileSourceError<DeserializeError>),
    Env(EnvSourceError),
}

type Loaded = LoadedConfig<'static, App, Origin, AppError>;

fn load(
    config: &std::path::Path,
    env: &[(&str, &str)],
    flags: ValuesSource,
    secrets: ValuesSource,
) -> Result<Loaded, ConfigLoadError<AppError>> {
    let given = || {
        App::register_source(
            |never: Infallible| match never {},
            |path, _: ValueDetail| Origin::Given(path),
        )
    };
    ConfigBuilder::<App, _, _>::new()
        .source_named(
            "config",
            App::register_source(AppError::File, |path, _: FileValueDetail| {
                Origin::File(path)
            }),
            yaml_file(config),
        )
        .source_named(
            "env",
            App::register_source(AppError::Env, |path, _: EnvVarDetail| Origin::Env(path)),
            EnvSource::from_vars(env.iter().map(|(k, v)| (k.to_string(), v.to_string())))
                .prefix("APP"),
        )
        // Last, so they override files and the environment.
        .source_named("flags", given(), flags)
        .source_named("flag files", given(), secrets)
        .load_with_meta()
}

const CONFIG: &str = "\
name: from-file
port: 80
debug: true
database:
  host: db.file
";

// r[verify values.source]
// r[verify values.strings]
// r[verify values.layer]
// r[verify load.layers]
#[test]
fn given_values_override_files_and_the_environment() {
    let dir = TempDir::new("values");
    let config = dir.write("config.yaml", CONFIG);
    let mut flags = ValuesSource::new();
    flags
        .insert(path("name"), "from-flag")
        .insert(path("debug"), "false");
    let mut secrets = ValuesSource::new().secrets();
    secrets.insert(path("database.password"), "hunter2");

    let loaded = load(&config, &[("APP__PORT", "8080")], flags, secrets).unwrap();
    assert_eq!(
        *loaded.config(),
        App {
            name: "from-flag".into(),
            port: 8080,
            debug: false,
            database: Database {
                host: "db.file".into(),
                password: "hunter2".into(),
            },
        }
    );
    let winner = |field: &str| loaded.source(&path(field)).unwrap().winner().clone();
    assert_eq!(
        winner("name"),
        ConfigValueSource::Direct(Origin::Given(path("name")))
    );
    assert_eq!(
        winner("port"),
        ConfigValueSource::Direct(Origin::Env(path("port")))
    );
}

// r[verify values.layer]
// r[verify values.unknown]
#[test]
fn given_values_are_checked_like_any_other() {
    let dir = TempDir::new("values-checked");
    let config = dir.write("config.yaml", CONFIG);
    let load = |flags: ValuesSource| match load(&config, &[], flags, ValuesSource::new().secrets())
        .unwrap_err()
    {
        ConfigLoadError::Violations(violations) => violations,
        error => panic!("expected violations, got {error:?}"),
    };

    // A path matching no field, such as a typo in the app's flag handling.
    let mut flags = ValuesSource::new();
    flags.insert(path("prot"), "8080");
    let violations = load(flags);
    assert_eq!(violations.unknown_paths()[0].path.to_string(), "prot");

    // A secret given to the public source.
    let mut flags = ValuesSource::new();
    flags.insert(path("database.password"), "hunter2");
    let violations = load(flags);
    assert_eq!(
        violations.layer_mismatches()[0].path.to_string(),
        "database.password"
    );
}

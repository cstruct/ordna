//! Loads the same config from every file format, and from a mix of formats.

mod common;

use std::path::PathBuf;

use common::*;
use ordna_core::{ConfigBuilder, ConfigValueSource};
use ordna_source_env::EnvSource;
use ordna_source_file::{FileFormat, FileSource};
use ordna_source_json::{Json, json_file};
use ordna_source_styx::{Styx, styx_file};
use ordna_source_toml::{Toml, toml_file};
use ordna_source_yaml::{Yaml, yaml_file};

/// [`DEFAULTS`] and [`SECRETS`] in each format.
const JSON_DEFAULTS: &str = r#"{
  "name": "ordna",
  "port": 8080,
  "debug": false,
  "db": { "host": "localhost", "port": 5432 },
  "log": { "level": "info" }
}"#;
const JSON_SECRETS: &str = r#"{
  "db": { "password": "hunter2" },
  "jwt": { "signing_key": "s3cr3t key", "ttl_seconds": 3600 }
}"#;

const TOML_DEFAULTS: &str = r#"
name = "ordna"
port = 8080
debug = false

[db]
host = "localhost"
port = 5432

[log]
level = "info"
"#;
const TOML_SECRETS: &str = r#"
[db]
password = "hunter2"

[jwt]
signing_key = "s3cr3t key"
ttl_seconds = 3600
"#;

const YAML_DEFAULTS: &str = "\
name: ordna
port: 8080
debug: false
db:
  host: localhost
  port: 5432
log:
  level: info
";
const YAML_SECRETS: &str = "\
db:
  password: hunter2
jwt:
  signing_key: s3cr3t key
  ttl_seconds: 3600
";

const ENV: [(&str, &str); 3] = [
    ("APP__PORT", "9090"),
    ("APP__DEBUG", "true"),
    ("APP__LOG__LEVEL", "debug"),
];

/// Loads defaults and secrets in format `F`, overridden by [`ENV`], and checks the values
/// and provenance.
fn check_format<F>(name: &str, defaults: &str, secrets: &str, open: fn(PathBuf) -> FileSource<F>)
where
    F: FileFormat<Error = ordna_source_styx::DeserializeError>,
{
    let dir = TempDir::new(&format!("format-{name}"));
    let defaults = dir.write(&format!("defaults.{name}"), defaults);
    let secrets_path = dir.write(&format!("secrets.{name}"), secrets);
    let loaded = ConfigBuilder::<App, _, _>::new()
        .source_named("defaults", file_registration(), open(defaults.clone()))
        .source_named(
            "secrets",
            file_registration(),
            open(secrets_path.clone()).secrets(),
        )
        .source_named(
            "env",
            env_registration(),
            EnvSource::from_vars(ENV).prefix("APP"),
        )
        .load_with_meta()
        .unwrap_or_else(|error| panic!("{name}: {error:?}"));

    assert_eq!(loaded.config(), &expected_app(), "{name}");

    let file = |file: &PathBuf, key: &str| {
        ConfigValueSource::Direct(Origin::File {
            file: file.clone(),
            key: key.into(),
            path: path(key),
        })
    };
    let port = loaded.source(&path("port")).unwrap();
    assert_eq!(port.shadowed(), &[file(&defaults, "port")], "{name}");
    assert_eq!(
        loaded.source(&path("jwt.ttl_seconds")).unwrap().winner(),
        &file(&secrets_path, "jwt.ttl_seconds"),
        "{name}"
    );
}

// r[verify format.json]
#[test]
fn json() {
    check_format::<Json>("json", JSON_DEFAULTS, JSON_SECRETS, json_file);
}

// r[verify format.toml]
#[test]
fn toml() {
    check_format::<Toml>("toml", TOML_DEFAULTS, TOML_SECRETS, toml_file);
}

// r[verify format.yaml]
#[test]
fn yaml() {
    check_format::<Yaml>("yaml", YAML_DEFAULTS, YAML_SECRETS, yaml_file);
}

// r[verify format.styx]
#[test]
fn styx() {
    check_format::<Styx>("styx", DEFAULTS, SECRETS, styx_file);
}

// r[verify format.mixed]
// r[verify load.precedence]
#[test]
fn formats_can_be_mixed() {
    let dir = TempDir::new("format-mixed");
    let defaults = dir.write("defaults.toml", TOML_DEFAULTS);
    let local = dir.write("local.yaml", "log:\n  level: warn\n  file: app.log\n");
    let overrides = dir.write("overrides.json", r#"{ "port": 7070 }"#);
    let secrets = dir.write("secrets.styx", SECRETS);

    let loaded = ConfigBuilder::<App, _, _>::new()
        .source_named("defaults", file_registration(), toml_file(&defaults))
        .source_named("local", file_registration(), yaml_file(&local))
        .source_named("overrides", file_registration(), json_file(&overrides))
        .source_named(
            "secrets",
            file_registration(),
            styx_file(&secrets).secrets(),
        )
        .load_with_meta()
        .unwrap();

    let app = loaded.config();
    assert_eq!(app.port, 7070);
    assert_eq!(app.log.level, "warn");
    assert_eq!(app.log.file.as_deref(), Some("app.log"));
    assert_eq!(app.database.password, "hunter2");
    assert_eq!(app.name, "ordna");

    let file = |file: &PathBuf, key: &str| {
        ConfigValueSource::Direct(Origin::File {
            file: file.clone(),
            key: key.into(),
            path: path(key),
        })
    };
    let port = loaded.source(&path("port")).unwrap();
    assert_eq!(port.winner(), &file(&overrides, "port"));
    assert_eq!(port.shadowed(), &[file(&defaults, "port")]);
    let level = loaded.source(&path("log.level")).unwrap();
    assert_eq!(level.winner(), &file(&local, "log.level"));
    assert_eq!(level.shadowed(), &[file(&defaults, "log.level")]);
    assert_eq!(
        loaded.source(&path("log.file")).unwrap().winner(),
        &file(&local, "log.file")
    );
}

//! Loads a config from a public Styx file, a secret Styx file and environment variables,
//! checking the loaded values, their provenance and the secrets rules.

mod common;

use common::*;
use ordna_core::{
    ConfigLayer, ConfigLoadError, ConfigValueSource, ConfigViolations, DiagnosticValue,
    SourceFieldPath,
};
use ordna_source_env::EnvSource;

const ENV: [(&str, &str); 4] = [
    ("APP__PORT", "9090"),
    ("APP__DEBUG", "true"),
    ("app__log__level", "debug"),
    ("PATH", "/usr/bin"),
];

fn env(vars: &[(&'static str, &'static str)]) -> EnvSource {
    EnvSource::from_vars(vars.iter().copied()).prefix("APP")
}

fn violations(result: LoadResult) -> ConfigViolations {
    match result {
        Err(ConfigLoadError::Violations(violations)) => violations,
        Err(ConfigLoadError::Source(error)) => panic!("unexpected source error: {error:?}"),
        Ok(loaded) => panic!("expected violations, loaded {:?}", loaded.config()),
    }
}

// r[verify load.precedence]
// r[verify load.decode.strings]
#[test]
fn loads_values_from_all_sources() {
    let dir = TempDir::new("values");
    let loaded = load(
        &dir.write("defaults.styx", DEFAULTS),
        &dir.write("secrets.styx", SECRETS),
        env(&ENV),
    )
    .unwrap();
    assert_eq!(loaded.config(), &expected_app());
}

// r[verify provenance.mapping]
// r[verify provenance.winner]
// r[verify provenance.shadowed]
// r[verify provenance.unset]
// r[verify provenance.list]
// r[verify env.detail]
// r[verify env.case-insensitive]
// r[verify file.detail]
#[test]
fn provenance_records_winners_and_shadowed_sources() {
    let dir = TempDir::new("provenance");
    let defaults = dir.write("defaults.styx", DEFAULTS);
    let secrets = dir.write("secrets.styx", SECRETS);
    let loaded = load(&defaults, &secrets, env(&ENV)).unwrap();

    let file = |file: &std::path::PathBuf, key: &str| {
        ConfigValueSource::Direct(Origin::File {
            file: file.clone(),
            key: key.into(),
            path: path(key),
        })
    };
    let from_env = |var: &str, key: &str| {
        ConfigValueSource::Direct(Origin::Env {
            var: var.into(),
            path: path(key),
        })
    };

    // Overridden by env: env wins, the defaults file is shadowed.
    let port = loaded.source(&path("port")).unwrap();
    assert_eq!(port.winner(), &from_env("APP__PORT", "port"));
    assert_eq!(port.shadowed(), &[file(&defaults, "port")]);
    assert_eq!(
        loaded.shadow_chain(&path("port")).unwrap(),
        &[file(&defaults, "port")]
    );
    assert_eq!(port.field_path(), &path("port"));
    assert_eq!(port.source_kind(), "env");

    // Variable names match case-insensitively, the provenance keeps the actual name.
    let level = loaded.source(&path("log.level")).unwrap();
    assert_eq!(level.winner(), &from_env("app__log__level", "log.level"));
    assert_eq!(level.shadowed(), &[file(&defaults, "log.level")]);

    // Only set by the defaults file.
    let name = loaded.source(&path("name")).unwrap();
    assert_eq!(name.winner(), &file(&defaults, "name"));
    assert!(name.shadowed().is_empty());
    assert_eq!(name.source_kind(), "file");

    // Secrets come from the secrets file, under their nested paths.
    let password = loaded.source(&path("db.password")).unwrap();
    assert_eq!(password.winner(), &file(&secrets, "db.password"));
    assert!(password.shadowed().is_empty());
    let signing_key = loaded.source(&path("jwt.signing_key")).unwrap();
    assert_eq!(signing_key.winner(), &file(&secrets, "jwt.signing_key"));

    // Fields no source set have no provenance.
    assert!(loaded.source(&path("log.file")).is_none());
    assert!(loaded.shadow_chain(&path("log.file")).is_none());
    // Neither do paths that aren't fields.
    assert!(loaded.source(&path("db")).is_none());

    // Every set field is listed, sorted by path, with consistent winners.
    let paths = loaded
        .sources()
        .iter()
        .map(|(path, _)| path.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            "db.host",
            "db.password",
            "db.port",
            "debug",
            "jwt.signing_key",
            "jwt.ttl_seconds",
            "log.level",
            "name",
            "port",
        ]
    );
    for (path, resolution) in loaded.sources() {
        assert_eq!(resolution.winner().field_path(), path);
        assert_eq!(loaded.source(path), Some(resolution));
    }
}

// r[verify env.public]
// r[verify load.layers]
// r[verify load.diagnostics.redact]
#[test]
fn env_cannot_set_secrets() {
    let dir = TempDir::new("env-secret");
    let violations = violations(load(
        &dir.write("defaults.styx", DEFAULTS),
        &dir.write("secrets.styx", SECRETS),
        env(&[
            ("APP__DB__PASSWORD", "leaked"),
            ("APP__JWT__TTL_SECONDS", "1"),
        ]),
    ));

    let mut issues = violations
        .layer_mismatches()
        .iter()
        .map(|issue| {
            (
                issue.path.to_string(),
                issue.source.name.as_deref(),
                issue.field_layer,
            )
        })
        .collect::<Vec<_>>();
    issues.sort();
    assert_eq!(
        issues,
        vec![
            ("db.password".into(), Some("env"), ConfigLayer::Secret),
            ("jwt.ttl_seconds".into(), Some("env"), ConfigLayer::Secret),
        ]
    );
    // The leaked values never show up in diagnostics.
    for issue in violations.layer_mismatches() {
        assert!(matches!(issue.value, DiagnosticValue::FullyRedacted));
    }
}

// r[verify file.layer]
// r[verify load.layers]
// r[verify load.diagnostics.redact]
#[test]
fn public_files_cannot_hold_secrets() {
    let dir = TempDir::new("public-secret");
    let defaults = format!("{DEFAULTS}jwt {{\n  signing_key leaked\n}}\n");
    let violations = violations(load(
        &dir.write("defaults.styx", &defaults),
        &dir.write("secrets.styx", SECRETS),
        env(&[]),
    ));

    let [issue] = violations.layer_mismatches() else {
        panic!("expected one issue: {violations:?}");
    };
    assert_eq!(issue.path, path("jwt.signing_key"));
    assert_eq!(issue.source.name.as_deref(), Some("defaults"));
    assert_eq!(issue.source.settings.layer, ConfigLayer::Public);
    assert_eq!(issue.field_layer, ConfigLayer::Secret);
    assert!(matches!(issue.value, DiagnosticValue::FullyRedacted));
}

// r[verify file.layer]
// r[verify load.layers]
// r[verify load.diagnostics.redact]
#[test]
fn secret_files_cannot_hold_public_fields() {
    let dir = TempDir::new("secret-public");
    let secrets = format!("{SECRETS}name other\n");
    let violations = violations(load(
        &dir.write("defaults.styx", DEFAULTS),
        &dir.write("secrets.styx", &secrets),
        env(&[]),
    ));

    let [issue] = violations.layer_mismatches() else {
        panic!("expected one issue: {violations:?}");
    };
    assert_eq!(issue.path, path("name"));
    assert_eq!(issue.source.name.as_deref(), Some("secrets"));
    assert_eq!(issue.source.settings.layer, ConfigLayer::Secret);
    assert_eq!(issue.field_layer, ConfigLayer::Public);
    // Values from secret sources are always redacted.
    assert!(matches!(issue.value, DiagnosticValue::FullyRedacted));
}

// r[verify load.unknown-paths]
// r[verify load.violations.collect]
// r[verify env.prefixed-unknown]
// r[verify file.unknown-keys]
#[test]
fn unknown_paths_are_reported_from_all_sources() {
    let dir = TempDir::new("unknown");
    let defaults = format!("{DEFAULTS}colour blue\n");
    let violations = violations(load(
        &dir.write("defaults.styx", &defaults),
        &dir.write("secrets.styx", SECRETS),
        env(&[("APP__LOG__LEVLE", "debug")]),
    ));

    let mut issues = violations
        .unknown_paths()
        .iter()
        .map(|issue| (issue.path.to_string(), issue.source.name.as_deref()))
        .collect::<Vec<_>>();
    issues.sort();
    assert_eq!(
        issues,
        vec![
            ("colour".into(), Some("defaults")),
            ("log.levle".into(), Some("env")),
        ]
    );
}

// r[verify load.source.error]
// r[verify file.missing]
#[test]
fn source_errors_name_the_failing_source() {
    let dir = TempDir::new("source-error");
    let missing = dir
        .write("secrets.styx", SECRETS)
        .with_file_name("missing.styx");
    let result = load(&missing, &dir.write("secrets.styx", SECRETS), env(&[]));

    let Err(ConfigLoadError::Source(error)) = result else {
        panic!("expected a source error");
    };
    assert_eq!(error.source.name.as_deref(), Some("defaults"));
    assert!(matches!(
        error.error,
        AppError::File(ordna_source_file::FileSourceError::Io { .. })
    ));
}

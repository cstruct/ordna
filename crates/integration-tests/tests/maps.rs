//! A config with a map of tagged backends whose entries are spread over a public file, a
//! secrets file and environment variables.

mod common;

use std::{collections::HashMap, path::PathBuf};

use common::{AppError, Origin, TempDir, path};
use ordna_core::{
    ConfigBuilder, ConfigLoadError, ConfigValueSource, SourceRegistration, config, config_tree,
};
use ordna_source_env::{EnvSource, EnvVarDetail};
use ordna_source_file::{FileFormat, FileSource, FileValueDetail};
use ordna_source_yaml::{DeserializeError, yaml_file};

#[config]
#[derive(Debug, PartialEq)]
#[facet(tag = "type", rename_all = "snake_case")]
pub enum Backend {
    S3 {
        bucket: String,
        region: String,
        endpoint: Option<String>,
        #[config(secret)]
        access_key_id: String,
        #[config(secret)]
        secret_access_key: String,
    },
    Swift {
        auth_url: String,
        container: String,
        #[config(secret)]
        password: String,
    },
}

config_tree! {
    #[derive(Debug, PartialEq)]
    pub struct Hylla {
        pub public_host: String,
        pub mappings: Vec<pub struct Mapping {
            pub backend: String,
            pub path_prefix: String,
            pub rewrite: Option<String>,
        }>,
        pub backends: HashMap<String, Backend>,
    }
}

const CONFIG: &str = "\
public_host: https://cache.example.com
mappings:
  - backend: assets
    path_prefix: /foo
    rewrite: /bar
  - backend: archive
    path_prefix: /
backends:
  assets:
    type: s3
    bucket: assets
    region: eu-north-1
  archive:
    type: swift
    auth_url: https://swift.example.com/v3
    container: archive
";

const SECRETS: &str = "\
backends:
  assets:
    access_key_id: AKIA123
    secret_access_key: s3cr3t
  archive:
    password: hunter2
";

fn file_registration<F>() -> SourceRegistration<FileSource<F>, Origin, AppError>
where
    F: FileFormat<Error = DeserializeError>,
{
    SourceRegistration::new(AppError::File, |path, detail: FileValueDetail| {
        Origin::File {
            file: detail.file,
            key: detail.key,
            path,
        }
    })
}

fn env_registration() -> SourceRegistration<EnvSource, Origin, AppError> {
    SourceRegistration::new(AppError::Env, |path, detail: EnvVarDetail| Origin::Env {
        var: detail.name,
        path,
    })
}

fn load(
    config: &PathBuf,
    secrets: &PathBuf,
    env: &[(&'static str, &'static str)],
) -> Result<ordna_core::LoadedConfig<'static, Hylla, Origin, AppError>, ConfigLoadError<AppError>> {
    ConfigBuilder::<Hylla, _, _>::new()
        .source_named("config", file_registration(), yaml_file(config))
        .source_named("secrets", file_registration(), yaml_file(secrets).secrets())
        .source_named(
            "env",
            env_registration(),
            EnvSource::from_vars(env.iter().copied()).prefix("HYLLA"),
        )
        .load_with_meta()
}

// r[verify load.merge]
// r[verify env.dynamic-keys]
// r[verify schema.fields.maps]
// r[verify schema.fields.enums]
#[test]
fn backends_merge_across_files_and_env() {
    let dir = TempDir::new("maps");
    let config = dir.write("config.yaml", CONFIG);
    let secrets = dir.write("secrets.yaml", SECRETS);
    let loaded = load(
        &config,
        &secrets,
        &[("HYLLA__BACKENDS__ASSETS__REGION", "eu-west-1")],
    )
    .unwrap();

    let hylla = loaded.config();
    assert_eq!(hylla.public_host, "https://cache.example.com");
    assert_eq!(
        hylla.mappings,
        vec![
            Mapping {
                backend: "assets".into(),
                path_prefix: "/foo".into(),
                rewrite: Some("/bar".into()),
            },
            Mapping {
                backend: "archive".into(),
                path_prefix: "/".into(),
                rewrite: None,
            },
        ]
    );
    assert_eq!(
        hylla.backends["assets"],
        Backend::S3 {
            bucket: "assets".into(),
            region: "eu-west-1".into(),
            endpoint: None,
            access_key_id: "AKIA123".into(),
            secret_access_key: "s3cr3t".into(),
        }
    );
    assert_eq!(
        hylla.backends["archive"],
        Backend::Swift {
            auth_url: "https://swift.example.com/v3".into(),
            container: "archive".into(),
            password: "hunter2".into(),
        }
    );

    // Every field of a map entry has its own provenance.
    let file = |file: &PathBuf, key: &str| {
        ConfigValueSource::Direct(Origin::File {
            file: file.clone(),
            key: key.into(),
            path: path(key),
        })
    };
    let region = loaded.source(&path("backends.assets.region")).unwrap();
    assert_eq!(
        region.winner(),
        &ConfigValueSource::Direct(Origin::Env {
            var: "HYLLA__BACKENDS__ASSETS__REGION".into(),
            path: path("backends.assets.region"),
        })
    );
    assert_eq!(
        region.shadowed(),
        &[file(&config, "backends.assets.region")]
    );
    assert_eq!(
        loaded
            .source(&path("backends.assets.secret_access_key"))
            .unwrap()
            .winner(),
        &file(&secrets, "backends.assets.secret_access_key")
    );
    // Lists are a single field.
    assert_eq!(
        loaded.source(&path("mappings")).unwrap().winner(),
        &file(&config, "mappings")
    );
}

// r[verify load.layers]
#[test]
fn backend_secrets_stay_out_of_public_sources() {
    let dir = TempDir::new("maps-secrets");
    let config = format!("{CONFIG}    password: leaked\n");
    let violations = match load(
        &dir.write("config.yaml", &config),
        &dir.write("secrets.yaml", SECRETS),
        &[("HYLLA__BACKENDS__ASSETS__SECRET_ACCESS_KEY", "leaked")],
    ) {
        Err(ConfigLoadError::Violations(violations)) => violations,
        other => panic!("expected violations, got {other:?}"),
    };
    let mut paths = violations
        .layer_mismatches()
        .iter()
        .map(|issue| (issue.path.to_string(), issue.source.name.clone().unwrap()))
        .collect::<Vec<_>>();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            ("backends.archive.password".into(), "config".into()),
            ("backends.assets.secret_access_key".into(), "env".into()),
        ]
    );
}

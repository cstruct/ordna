//! Partial loads: when loading fails with violations, the values set so far, with their
//! provenance and the defaults filled in, along with the required fields still missing.

use std::{collections::HashMap, convert::Infallible};

use facet::Facet;
use facet_value::{Value, value};
use ordna_core::{
    ConfigBuilder, ConfigPath, MissingField, PartialConfig, PartialLoadError, SourceFieldPath,
    SourceRegistration, ValueDetail, ValuesSource, config, config_contexts, config_tree,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Given(ConfigPath);

impl SourceFieldPath for Given {
    fn field_path(&self) -> &ConfigPath {
        &self.0
    }
}

fn path(path: &str) -> ConfigPath {
    ConfigPath::parse(path).unwrap()
}

fn values(values: &[(&str, &str)]) -> ValuesSource {
    values
        .iter()
        .map(|(path, value)| (self::path(path), value.to_string()))
        .collect()
}

fn missing(partial: &PartialConfig<Given>) -> Vec<(Option<&'static str>, String)> {
    partial
        .missing_fields()
        .iter()
        .map(|MissingField { context, path }| (*context, path.to_string()))
        .collect()
}

#[derive(Debug, PartialEq, Facet)]
#[repr(C)]
#[facet(tag = "type", rename_all = "snake_case")]
pub enum Backend {
    S3 {
        bucket: String,
        region: Option<String>,
    },
    Local {
        root: String,
    },
}

#[derive(Debug, PartialEq, Facet)]
#[facet(default)]
pub struct Limits {
    pub rps: u32,
    pub burst: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self { rps: 10, burst: 20 }
    }
}

config_tree! {
    #[derive(Debug, PartialEq)]
    pub struct App {
        pub name: String,
        pub http: pub struct Http {
            pub port: u16,
            #[facet(default = 30)]
            pub timeout_seconds: u32,
            pub tls: Option<String>,
            pub tags: Vec<String>,
        },
        pub limits: Limits,
        pub backends: HashMap<String, Backend>,
    }
}

fn load_app(given: &[(&str, &str)]) -> Result<App, PartialLoadError<Given, Infallible>> {
    ConfigBuilder::<App, Given, Infallible>::new()
        .source(
            SourceRegistration::new(|never| match never {}, |path, _: ValueDetail| Given(path)),
            values(given),
        )
        .load_with_meta_or_partial()
        .map(|loaded| loaded.into_config())
}

fn partial<T: std::fmt::Debug>(
    result: Result<T, PartialLoadError<Given, Infallible>>,
) -> PartialConfig<Given> {
    match result {
        Err(PartialLoadError::Violations(partial)) => *partial,
        other => panic!("expected violations, got {other:?}"),
    }
}

// r[verify load.partial]
// r[verify load.violations.missing]
#[test]
fn partial_loads_keep_the_values_set_and_report_missing_fields() {
    let partial = partial(load_app(&[
        ("http.tls", "on"),
        ("backends.eu.type", "s3"),
        ("backends.disk.type", "local"),
        ("backends.disk.root", "/srv"),
    ]));
    assert_eq!(
        missing(&partial),
        [
            (None, "name".to_string()),
            (None, "http.port".to_string()),
            (None, "backends.eu.bucket".to_string()),
        ]
    );
    // Every missing field is a decode violation too.
    assert_eq!(partial.violations().decode_errors().len(), 3);

    let backends = partial
        .value()
        .as_object()
        .unwrap()
        .get("backends")
        .unwrap();
    assert_eq!(
        backends,
        &value!({
            "eu": { "type": "s3", "region": null },
            "disk": { "type": "local", "root": "/srv" },
        })
    );
    let winner = partial.source(&path("http.tls")).unwrap().winner();
    assert_eq!(
        winner,
        &ordna_core::ConfigValueSource::Direct(Given(path("http.tls")))
    );
    assert!(partial.source(&path("http.port")).is_none());
    assert_eq!(partial.sources().len(), 4);
}

// r[verify load.partial.defaults]
#[test]
fn partial_values_have_the_defaults_filled_in() {
    let first = partial(load_app(&[("name", "app")]));
    assert_eq!(missing(&first), [(None, "http".to_string())]);
    assert_eq!(
        first.value(),
        &value!({
            "name": "app",
            "limits": { "rps": 10, "burst": 20 },
            "backends": {},
        })
    );

    let partial = self::partial(load_app(&[("name", "app"), ("http.tls", "on")]));
    assert_eq!(missing(&partial), [(None, "http.port".to_string())]);
    let http = partial.value().as_object().unwrap().get("http").unwrap();
    assert_eq!(
        http,
        &value!({ "tls": "on", "timeout_seconds": 30, "tags": [] })
    );
}

// r[verify load.partial.violations]
#[test]
fn partial_loads_report_the_violations_a_load_does() {
    // An unknown path hides decode violations, as it does for a load, but the values set
    // are still there.
    let partial = partial(load_app(&[("name", "app"), ("http.prot", "8080")]));
    assert_eq!(partial.violations().unknown_paths().len(), 1);
    assert!(partial.violations().decode_errors().is_empty());
    assert!(partial.missing_fields().is_empty());
    assert_eq!(
        partial.value().as_object().unwrap().get("name"),
        Some(&Value::from("app"))
    );
}

#[test]
fn successful_loads_are_unchanged() {
    let app = load_app(&[("name", "app"), ("http.port", "8080")]).unwrap();
    assert_eq!(app.http.port, 8080);
    assert_eq!(app.http.timeout_seconds, 30);
    assert_eq!(app.limits, Limits::default());
}

#[config]
#[derive(Debug, PartialEq)]
pub struct Web {
    pub port: u16,
    #[config(path = "db")]
    pub database: Option<Db>,
}

config_tree! {
    #[derive(Debug, PartialEq)]
    pub struct Worker {
        pub db: pub struct Db {
            pub host: String,
            #[facet(default = 5)]
            pub pool: u32,
        },
        pub queue: String,
    }
}

config_contexts!(pub Contexts = [Web, Worker]);

// r[verify contexts.partial]
// r[verify contexts.decode.collect]
#[test]
fn partial_contexts_merge_their_values_and_name_missing_fields_by_context() {
    let result = ConfigBuilder::<Contexts, Given, Infallible>::new()
        .source(
            Contexts::register_source(|never| match never {}, |path, _: ValueDetail| Given(path)),
            values(&[("port", "8080"), ("db.host", "db.internal")]),
        )
        .load_contexts_with_meta_or_partial::<(Web, Worker)>();
    let partial = partial(result.map(|loaded| loaded.into_config()));
    assert_eq!(missing(&partial), [(Some("Worker"), "queue".to_string())]);
    assert_eq!(
        partial.value(),
        &value!({
            "port": "8080",
            "db": { "host": "db.internal", "pool": 5 },
        })
    );
}

//! An app with several entrypoints, each loading only the config contexts it needs from
//! sources checked against the fields of all of them.

use std::convert::Infallible;

use ordna_core::{
    ConfigBuilder, ConfigContexts, ConfigLoadError, ConfigPath, ConfigViolations, LoadedConfig,
    SourceFieldPath, ValueDetail, ValuesSource, config, config_contexts, config_tree,
};

#[config]
#[derive(Debug, PartialEq)]
pub struct Http {
    pub port: u16,
    pub public_url: String,
}

config_tree! {
    #[derive(Debug, PartialEq)]
    pub struct Db {
        #[config(path = "db")]
        pub database: pub struct Database {
            pub host: String,
            #[config(secret)]
            pub password: String,
        },
    }
}

/// The database for entrypoints that can run without one.
#[config]
#[derive(Debug, PartialEq)]
pub struct DbOpt {
    #[config(path = "db")]
    pub database: Option<Database>,
}

// The database host alone, with the password declared public.
config_tree! {
    #[derive(Debug, PartialEq)]
    pub struct DbHost {
        pub db: pub struct DbHostFields {
            pub host: String,
            pub password: Option<String>,
        },
    }
}

config_tree! {
    #[derive(Debug, PartialEq)]
    pub struct Mailer {
        pub smtp: pub struct Smtp {
            pub host: String,
            #[config(secret)]
            pub password: String,
        },
    }
}

config_contexts!(pub AppContexts = [Http, Db, DbOpt, DbHost, Mailer]);

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

type Loaded<X> = LoadedConfig<'static, X, Given, Infallible>;

/// Loads contexts `X` from a public and a secret source of the given values.
fn load<X: ConfigContexts<AppContexts>>(
    public: &[(&str, &str)],
    secrets: &[(&str, &str)],
) -> Result<Loaded<X>, ConfigLoadError<Infallible>> {
    let given = || {
        AppContexts::register_source(
            |never: Infallible| match never {},
            |path, _: ValueDetail| Given(path),
        )
    };
    ConfigBuilder::<AppContexts, _, _>::new()
        .source_named("public", given(), values(public))
        .source_named("secrets", given(), values(secrets).secrets())
        .load_contexts_with_meta()
}

fn violations<X: std::fmt::Debug>(
    result: Result<Loaded<X>, ConfigLoadError<Infallible>>,
) -> ConfigViolations {
    match result {
        Err(ConfigLoadError::Violations(violations)) => violations,
        other => panic!("expected violations, got {other:?}"),
    }
}

const PUBLIC: &[(&str, &str)] = &[
    ("port", "8080"),
    ("public_url", "https://example.com"),
    ("db.host", "db.internal"),
    ("smtp.host", "smtp.internal"),
];
const SECRETS: &[(&str, &str)] = &[("db.password", "hunter2"), ("smtp.password", "swordfish")];

// r[verify macro.contexts]
// r[verify contexts.schema]
#[test]
fn entrypoints_load_the_contexts_they_need() {
    let (http, db, mailer) = load::<(Http, Db, Mailer)>(PUBLIC, SECRETS)
        .unwrap()
        .into_config();
    assert_eq!(
        http,
        Http {
            port: 8080,
            public_url: "https://example.com".into(),
        }
    );
    assert_eq!(db.database.password, "hunter2");
    assert_eq!(mailer.smtp.password, "swordfish");
}

// r[verify contexts.validate]
// r[verify contexts.decode]
#[test]
fn fields_of_other_contexts_are_accepted_but_not_required() {
    // The migration job needs the database only: the mailer's fields are known, and
    // the mailer's missing password doesn't matter.
    let secrets = &[("db.password", "hunter2")];
    let (db,) = load::<(Db,)>(PUBLIC, secrets).unwrap().into_config();
    assert_eq!(db.database.host, "db.internal");

    // A typo is still an unknown path.
    let mut public = PUBLIC.to_vec();
    public.push(("db.hots", "db.internal"));
    let violations = violations(load::<(Db,)>(&public, secrets));
    let [unknown] = violations.unknown_paths() else {
        panic!("expected one unknown path, got {violations:?}");
    };
    assert_eq!(unknown.path, path("db.hots"));
}

// r[verify contexts.decode]
#[test]
fn contexts_sharing_fields_can_require_them_differently() {
    let (db,) = load::<(DbOpt,)>(&[("port", "8080")], &[])
        .unwrap()
        .into_config();
    assert_eq!(db, DbOpt { database: None });

    let violations = violations(load::<(Db,)>(&[("port", "8080")], &[]));
    assert_eq!(violations.decode_errors().len(), 1);
}

// r[verify contexts.decode.collect]
#[test]
fn decode_violations_of_every_context_are_reported() {
    let violations = violations(load::<(Http, Db, Mailer)>(&[], &[]));
    let errors = violations
        .decode_errors()
        .iter()
        .map(|error| error.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        errors,
        [
            "Http: missing required field `port`",
            "Http: missing required field `public_url`",
            "Db: missing required field `db`",
            "Mailer: missing required field `smtp`",
        ]
    );
}

// r[verify contexts.schema.secret]
#[test]
fn fields_secret_in_any_context_are_secret() {
    // `DbHost` declares `db.password` public, but `Db` makes it secret.
    let (db,) = load::<(DbHost,)>(&[("db.host", "db.internal")], &[("db.password", "hunter2")])
        .unwrap()
        .into_config();
    assert_eq!(db.db.password.as_deref(), Some("hunter2"));

    let violations = violations(load::<(DbHost,)>(
        &[("db.host", "db.internal"), ("db.password", "hunter2")],
        &[],
    ));
    assert_eq!(violations.layer_mismatches().len(), 1);
}

// r[verify contexts.provenance]
#[test]
fn provenance_lists_the_fields_of_loaded_contexts() {
    let loaded = load::<(Http,)>(PUBLIC, SECRETS).unwrap();
    assert_eq!(
        loaded.source(&path("port")).unwrap().winner().field_path(),
        &path("port")
    );
    assert!(loaded.source(&path("db.host")).is_none());
    assert_eq!(loaded.sources().len(), 2);
}

#[test]
fn a_single_context_loads_without_a_tuple() {
    let http = ConfigBuilder::<AppContexts, Given, Infallible>::new()
        .source(
            AppContexts::register_source(|never| match never {}, |path, _| Given(path)),
            values(PUBLIC),
        )
        .load_context::<Http>()
        .unwrap();
    assert_eq!(http.port, 8080);
}

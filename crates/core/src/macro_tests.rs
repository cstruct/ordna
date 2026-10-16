use facet_value::Value;

use crate::{
    ConfigLayer, ConfigLoadError, ConfigModel, ConfigPath, ConfigPathSegment, ConfigSchema,
    ConfigSource, ConfigSourceSettings, DiagnosticValue, FieldCandidate, LoadedConfig, Schema,
    SourceFieldPath, SourceRegistration, config, config_tree,
    engine::load_config_from_sources,
    source::{binding::BoundSource, field_candidate::SourceValueDetail},
};

struct NoPath;
impl SourceFieldPath for NoPath {
    fn field_path(&self) -> &ConfigPath {
        unimplemented!()
    }
}

#[derive(Debug)]
struct Detail;
impl SourceValueDetail for Detail {
    fn label(&self) -> String {
        "test".into()
    }
}

struct TestSource(ConfigLayer, Vec<(&'static str, &'static str)>);

impl ConfigSource for TestSource {
    type Error = String;
    type Detail = Detail;
    fn settings(&self) -> ConfigSourceSettings {
        let builder = ConfigSourceSettings::builder();
        match self.0 {
            ConfigLayer::Public => builder.public(),
            ConfigLayer::Secret => builder.secrets(),
        }
        .build()
    }
    fn load(&self, _schema: &'static Schema) -> Result<Vec<FieldCandidate<Detail>>, String> {
        Ok(self
            .1
            .iter()
            .map(|(path, value)| FieldCandidate::new(path_of(path), Value::from(*value), Detail))
            .collect())
    }
}

fn path_of(path: &str) -> ConfigPath {
    ConfigPath::new(
        path.split('.')
            .map(|segment| ConfigPathSegment::parse(segment.to_string()).unwrap())
            .collect(),
    )
}

fn fields<T: ConfigModel<'static, NoPath, String>>() -> Vec<String> {
    T::schema()
        .fields()
        .keys()
        .map(ToString::to_string)
        .collect()
}

type Values = Vec<(&'static str, &'static str)>;

fn registration() -> SourceRegistration<TestSource, NoPath, String> {
    SourceRegistration::new(|error| error, |_, _| NoPath)
}

fn try_load<T: ConfigModel<'static, NoPath, String>>(
    sources: Vec<(ConfigLayer, Values)>,
) -> Result<T, ConfigLoadError<String>> {
    let sources = sources
        .into_iter()
        .map(|(layer, values)| {
            BoundSource::new_anonymous(TestSource(layer, values), registration())
        })
        .collect();
    load_config_from_sources::<T, NoPath, String>(sources).map(LoadedConfig::into_config)
}

fn load<T: ConfigModel<'static, NoPath, String>>(values: Values) -> T {
    try_load(vec![(ConfigLayer::Public, values)]).unwrap()
}

mod attribute {
    use super::*;

    #[config]
    #[derive(Debug, PartialEq)]
    pub struct App {
        pub name: String,
        #[config(path = "token")]
        pub jwt: Jwt,
    }

    #[config]
    #[derive(Debug, PartialEq)]
    pub struct Jwt {
        #[config(path = "ttl")]
        pub ttl_seconds: u32,
        pub issuer: Option<String>,
    }

    // r[verify macro.attribute]
    // r[verify macro.path]
    // r[verify schema.fields.names]
    #[test]
    fn paths_follow_overrides() {
        assert_eq!(fields::<App>(), vec!["name", "token.issuer", "token.ttl"]);
    }

    // r[verify macro.attribute]
    // r[verify macro.path]
    #[test]
    fn loads_with_overridden_paths() {
        let app = load::<App>(vec![("name", "app"), ("token.ttl", "60")]);
        assert_eq!(
            app,
            App {
                name: "app".into(),
                jwt: Jwt {
                    ttl_seconds: 60,
                    issuer: None,
                },
            }
        );
    }
}

mod tree {
    use super::*;

    config_tree! {
        #[derive(Debug, PartialEq)]
        pub struct App {
            pub name: String,
            #[config(path = "token")]
            pub jwt: pub struct Jwt {
                #[config(path = "ttl")]
                pub ttl_seconds: u32,
                pub issuer: Option<String>,
                pub keys: pub struct Keys {
                    pub public: String,
                },
            },
            pub r#type: String,
            pub tags: Option<Vec<String>>,
        }
    }

    // r[verify macro.tree]
    // r[verify macro.raw-idents]
    // r[verify schema.fields.leaves]
    // r[verify schema.fields.names]
    #[test]
    fn paths_follow_overrides() {
        assert_eq!(
            fields::<App>(),
            vec![
                "name",
                "tags",
                "token.issuer",
                "token.keys.public",
                "token.ttl",
                "type"
            ]
        );
    }

    // r[verify macro.tree]
    // r[verify macro.tree.derives]
    #[test]
    fn nested_structs_are_loadable_types() {
        let app = load::<App>(vec![
            ("name", "app"),
            ("token.ttl", "60"),
            ("token.keys.public", "pk"),
            ("type", "web"),
        ]);
        assert_eq!(
            app.jwt,
            Jwt {
                ttl_seconds: 60,
                issuer: None,
                keys: Keys {
                    public: "pk".into(),
                },
            }
        );
        // Nested structs are config models of their own.
        assert_eq!(fields::<Keys>(), vec!["public"]);
    }
}

mod secrets {
    use super::*;

    config_tree! {
        #[derive(Debug, PartialEq)]
        pub struct App {
            pub name: String,
            #[config(secret)]
            pub password: String,
            #[config(secret, path = "keys")]
            pub signing: pub struct Signing {
                pub private: String,
            },
        }
    }

    fn layer(path: &str) -> Option<ConfigLayer> {
        <App as ConfigSchema>::schema().layer(&path_of(path))
    }

    // r[verify macro.secret]
    // r[verify schema.layer.public]
    // r[verify schema.layer.secret]
    // r[verify schema.layer.inherit]
    #[test]
    fn secret_fields_and_their_children_are_secret() {
        assert_eq!(layer("name"), Some(ConfigLayer::Public));
        assert_eq!(layer("password"), Some(ConfigLayer::Secret));
        assert_eq!(layer("keys.private"), Some(ConfigLayer::Secret));
        assert_eq!(layer("signing.private"), None);
    }

    // r[verify load.layers]
    #[test]
    fn secrets_load_from_secret_sources() {
        let app = try_load::<App>(vec![
            (ConfigLayer::Public, vec![("name", "app")]),
            (
                ConfigLayer::Secret,
                vec![("password", "hunter2"), ("keys.private", "pk")],
            ),
        ])
        .unwrap();
        assert_eq!(app.password, "hunter2");
        assert_eq!(app.signing.private, "pk");
    }

    fn mismatches(sources: Vec<(ConfigLayer, Values)>) -> Vec<(String, ConfigLayer, bool)> {
        let Err(ConfigLoadError::Violations(violations)) = try_load::<App>(sources) else {
            panic!("expected violations");
        };
        violations
            .layer_mismatches()
            .iter()
            .map(|issue| {
                (
                    issue.path.to_string(),
                    issue.field_layer,
                    matches!(issue.value, DiagnosticValue::FullyRedacted),
                )
            })
            .collect()
    }

    // r[verify load.layers]
    // r[verify load.diagnostics.redact]
    #[test]
    fn public_sources_cannot_set_secrets() {
        let issues = mismatches(vec![(
            ConfigLayer::Public,
            vec![
                ("name", "app"),
                ("password", "hunter2"),
                ("keys.private", "pk"),
            ],
        )]);
        // The leaked values are redacted in diagnostics.
        assert_eq!(
            issues,
            vec![
                ("password".into(), ConfigLayer::Secret, true),
                ("keys.private".into(), ConfigLayer::Secret, true),
            ]
        );
    }

    // r[verify load.layers]
    // r[verify load.diagnostics.redact]
    #[test]
    fn secret_sources_cannot_set_public_fields() {
        let issues = mismatches(vec![(
            ConfigLayer::Secret,
            vec![
                ("name", "app"),
                ("password", "hunter2"),
                ("keys.private", "pk"),
            ],
        )]);
        assert_eq!(issues, vec![("name".into(), ConfigLayer::Public, true)]);
    }
}

mod maps {
    use std::collections::HashMap;

    use super::*;

    #[config]
    #[derive(Debug, PartialEq)]
    #[facet(tag = "type", rename_all = "snake_case")]
    #[repr(C)]
    pub enum Backend {
        S3 {
            bucket: String,
            region: Option<String>,
            #[config(secret)]
            secret_access_key: String,
        },
        Swift {
            container: String,
            #[config(secret)]
            password: String,
        },
    }

    #[config]
    #[derive(Debug, PartialEq)]
    pub struct Hook {
        pub url: String,
        #[config(secret)]
        pub token: String,
    }

    #[config]
    #[derive(Debug)]
    pub struct App {
        pub backends: HashMap<String, Backend>,
        pub labels: HashMap<String, String>,
        pub tags: Vec<String>,
        pub hooks: Option<Vec<Hook>>,
    }

    fn layers() -> Vec<(String, ConfigLayer)> {
        <App as ConfigSchema>::schema()
            .fields()
            .iter()
            .map(|(pattern, layer)| (pattern.to_string(), *layer))
            .collect()
    }

    // r[verify schema.fields.maps]
    // r[verify schema.fields.enums]
    // r[verify schema.layer.nested-leaves]
    // r[verify macro.enum]
    #[test]
    fn maps_and_tagged_enums_are_walked() {
        use ConfigLayer::{Public, Secret};
        assert_eq!(
            layers(),
            vec![
                ("backends.*.bucket".into(), Public),
                ("backends.*.container".into(), Public),
                ("backends.*.password".into(), Secret),
                ("backends.*.region".into(), Public),
                ("backends.*.secret_access_key".into(), Secret),
                ("backends.*.type".into(), Public),
                // A list holding secrets is secret as a whole.
                ("hooks".into(), Secret),
                ("labels.*".into(), Public),
                ("tags".into(), Public),
            ]
        );
    }

    // r[verify load.merge]
    // r[verify schema.fields.maps]
    #[test]
    fn map_entries_merge_public_and_secret_sources() {
        let app = try_load::<App>(vec![
            (
                ConfigLayer::Public,
                vec![
                    ("backends.eu-west.type", "s3"),
                    ("backends.eu-west.bucket", "assets"),
                    ("backends.archive.type", "swift"),
                    ("backends.archive.container", "old"),
                    ("labels.team", "storage"),
                ],
            ),
            (
                ConfigLayer::Secret,
                vec![
                    ("backends.eu-west.secret_access_key", "s3cr3t"),
                    ("backends.archive.password", "hunter2"),
                ],
            ),
        ])
        .unwrap();
        assert_eq!(
            app.backends["eu-west"],
            Backend::S3 {
                bucket: "assets".into(),
                region: None,
                secret_access_key: "s3cr3t".into(),
            }
        );
        assert_eq!(
            app.backends["archive"],
            Backend::Swift {
                container: "old".into(),
                password: "hunter2".into(),
            }
        );
        assert_eq!(app.labels["team"], "storage");
    }

    // r[verify load.layers]
    // r[verify load.unknown-paths]
    #[test]
    fn map_entries_follow_the_secrets_rules() {
        let Err(ConfigLoadError::Violations(violations)) = try_load::<App>(vec![(
            ConfigLayer::Public,
            vec![
                ("backends.eu-west.type", "s3"),
                ("backends.eu-west.bucket", "assets"),
                ("backends.eu-west.secret_access_key", "leaked"),
                ("backends.eu-west.bucke", "typo"),
            ],
        )]) else {
            panic!("expected violations");
        };
        let mismatches = violations
            .layer_mismatches()
            .iter()
            .map(|issue| issue.path.to_string())
            .collect::<Vec<_>>();
        assert_eq!(mismatches, vec!["backends.eu-west.secret_access_key"]);
        let unknown = violations
            .unknown_paths()
            .iter()
            .map(|issue| issue.path.to_string())
            .collect::<Vec<_>>();
        assert_eq!(unknown, vec!["backends.eu-west.bucke"]);
    }
}

mod tree_generics {
    use std::collections::HashMap;

    use super::*;

    config_tree! {
        #[derive(Debug, PartialEq)]
        pub struct App {
            pub mappings: Vec<pub struct Mapping {
                pub path_prefix: String,
            }>,
            pub backends: HashMap<String, #[facet(tag = "type", rename_all = "snake_case")] pub enum Backend {
                S3 {
                    bucket: String,
                    #[config(secret, path = "secret")]
                    secret_access_key: String,
                },
                Local,
            }>,
        }
    }

    // r[verify macro.tree.generics]
    // r[verify macro.tree.derives]
    // r[verify macro.tree.enums]
    #[test]
    fn declarations_in_generic_arguments() {
        let schema = <App as ConfigSchema>::schema();
        let fields = schema
            .fields()
            .iter()
            .map(|(pattern, layer)| (pattern.to_string(), *layer))
            .collect::<Vec<_>>();
        assert_eq!(
            fields,
            vec![
                ("backends.*.bucket".into(), ConfigLayer::Public),
                ("backends.*.secret".into(), ConfigLayer::Secret),
                ("backends.*.type".into(), ConfigLayer::Public),
                ("mappings".into(), ConfigLayer::Public),
            ]
        );

        let app = try_load::<App>(vec![
            (
                ConfigLayer::Public,
                vec![
                    ("backends.a.type", "s3"),
                    ("backends.a.bucket", "assets"),
                    ("backends.b.type", "local"),
                ],
            ),
            (ConfigLayer::Secret, vec![("backends.a.secret", "s3cr3t")]),
        ])
        .unwrap();
        // The derives were passed on to the nested declarations.
        assert_eq!(
            app,
            App {
                mappings: Vec::new(),
                backends: HashMap::from([
                    (
                        "a".into(),
                        Backend::S3 {
                            bucket: "assets".into(),
                            secret_access_key: "s3cr3t".into(),
                        }
                    ),
                    ("b".into(), Backend::Local),
                ]),
            }
        );
        let _ = Mapping {
            path_prefix: "/".into(),
        };
    }
}

pub(crate) mod tree;

use std::collections::{BTreeMap, btree_map::Entry};

use facet_value::{VObject, Value, from_value};

use crate::{
    ConfigLayer, ConfigLoadError, ConfigModel, ConfigPath, ConfigValueSource, FieldCandidate,
    LoadedConfig, SourceFieldPath, SourceResolution,
    error::{ConfigViolations, DecodeError},
    issue::{ConflictIssue, LayerMismatchIssue},
    source::binding::BoundSource,
};

/// Loads `T` from `sources`.
///
/// Sources are applied in order, so a value from a later source shadows the same field
/// from an earlier one; the provenance of every set field records the winning source and
/// the shadowed ones, most recent first. A source may only provide values for fields of its own layer:
/// public sources for public fields, secret sources for secret fields. All violations
/// (unknown paths, conflicts within one source, layer mismatches and decode failures) are
/// collected and returned together.
pub(crate) fn load_config_from_sources<T, P, E>(
    sources: Vec<BoundSource<P, E>>,
) -> Result<LoadedConfig<'static, T, P, E>, ConfigLoadError<E>>
where
    E: Send + Sync + 'static,
    T: ConfigModel<'static, P, E>,
    P: SourceFieldPath,
{
    let schema = T::schema();
    let mut violations = ConfigViolations::default();

    // r[impl load.precedence]
    // Every candidate per path, in source order. The last one wins.
    let mut candidates = BTreeMap::<ConfigPath, Vec<FieldCandidate<P>>>::new();
    for source in &sources {
        let source_candidates = load_config_from_source(source, schema, &mut violations)?;
        for candidate in source_candidates {
            candidates
                .entry(candidate.path().clone())
                .or_default()
                .push(candidate);
        }
    }

    let mut root = VObject::new();
    for (path, contenders) in &candidates {
        let winner = contenders.last().expect("entries always hold a candidate");
        insert_nested(&mut root, path, winner.value().clone());
    }

    // r[impl load.violations.collect]
    if violations.has_violations() {
        return Err(ConfigLoadError::Violations(violations));
    }

    // r[impl load.decode]
    // r[impl load.decode.strings]
    let config = from_value::<T>(Value::from(root)).map_err(|error| {
        let mut violations = ConfigViolations::default();
        violations.add_decode_violation(DecodeError {
            message: error.to_string(),
        });
        ConfigLoadError::Violations(violations)
    })?;

    // r[impl provenance.winner]
    // r[impl provenance.shadowed]
    // r[impl provenance.list]
    let sources = candidates
        .into_iter()
        .map(|(path, contenders)| {
            let mut origins = contenders
                .into_iter()
                .rev()
                .map(|candidate| ConfigValueSource::Direct(candidate.into_detail()));
            let winner = origins.next().expect("entries always hold a candidate");
            (path, SourceResolution::new(winner, origins.collect()))
        })
        .collect();
    Ok(LoadedConfig::new(config, sources))
}

/// Loads the candidates of one source, dropping (or reporting) unknown paths and
/// reporting values for fields of another layer and paths the source provided more than
/// once.
fn load_config_from_source<P, E>(
    bound_source: &BoundSource<P, E>,
    schema: &'static crate::Schema,
    violations: &mut ConfigViolations,
) -> Result<Vec<FieldCandidate<P>>, ConfigLoadError<E>>
where
    E: Send + Sync + 'static,
{
    let source_meta = &bound_source.meta;
    let allow_unknown_paths = source_meta.settings.allow_unknown_paths;
    let source_values = bound_source.load(schema)?;

    let mut field_values = BTreeMap::<ConfigPath, Vec<FieldCandidate<P>>>::new();
    for field_candidate in source_values {
        // r[impl load.unknown-paths]
        // r[impl load.unknown-paths.allow]
        let Some(field_layer) = schema.layer(field_candidate.path()) else {
            if !allow_unknown_paths {
                violations.add_unknown_path_violation(
                    field_candidate.path().clone(),
                    source_meta.clone(),
                    field_candidate.as_diagnostic_value(source_meta, ConfigLayer::Public),
                );
            }
            continue;
        };
        // r[impl load.layers]
        if field_layer != source_meta.settings.layer {
            violations.add_layer_mismatch_violation(LayerMismatchIssue {
                path: field_candidate.path().clone(),
                source: source_meta.clone(),
                field_layer,
                value: field_candidate.as_diagnostic_value(source_meta, field_layer),
            });
            continue;
        }
        match field_values.entry(field_candidate.path().clone()) {
            Entry::Vacant(entry) => {
                entry.insert(vec![field_candidate]);
            }
            Entry::Occupied(mut entry) => entry.get_mut().push(field_candidate),
        }
    }

    let mut accepted = Vec::with_capacity(field_values.len());
    for (path, mut values) in field_values {
        // r[impl load.conflicts]
        if values.len() > 1 {
            let field_layer = schema.layer(&path).expect("only known paths are kept");
            violations.add_conflict_violation(ConflictIssue {
                path,
                source: source_meta.clone(),
                contenders: values
                    .iter()
                    .map(|value| value.as_diagnostic_value(source_meta, field_layer))
                    .collect(),
            });
            continue;
        }
        accepted.push(values.pop().expect("entries always hold a candidate"));
    }
    Ok(accepted)
}

// r[impl load.merge]
/// Inserts `value` into `root` at `path`, creating intermediate objects.
fn insert_nested(root: &mut VObject, path: &ConfigPath, value: Value) {
    let segments = path.segments();
    let (leaf, parents) = segments.split_last().expect("config paths are never empty");
    let mut node = root;
    for segment in parents {
        let key = segment.to_string();
        let is_object = node
            .get(&key)
            .is_some_and(|value| value.as_object().is_some());
        if !is_object {
            node.insert(key.as_str(), VObject::new());
        }
        node = node
            .get_mut(&key)
            .and_then(Value::as_object_mut)
            .expect("just inserted an object");
    }
    node.insert(leaf.to_string(), value);
}

#[cfg(test)]
mod tests {
    use facet::Facet;
    use facet_value::Value;

    use super::load_config_from_sources;
    use crate::{
        ConfigLoadError, ConfigModel, ConfigPath, ConfigPathSegment, ConfigSource,
        ConfigSourceSettings, FieldCandidate, Schema, SourceFieldPath, SourceRegistration,
        source::{binding::BoundSource, field_candidate::SourceValueDetail},
    };

    #[derive(Facet, Debug, PartialEq)]
    struct Jwt {
        ttl_seconds: u32,
        issuer: Option<String>,
    }

    #[derive(Facet, Debug, PartialEq)]
    struct App {
        name: String,
        jwt: Jwt,
    }

    struct NoPath;
    impl SourceFieldPath for NoPath {
        fn field_path(&self) -> &ConfigPath {
            unimplemented!()
        }
    }

    static SCHEMA: Schema = Schema::new::<App>();

    impl ConfigModel<'static, NoPath, String> for App {
        fn schema() -> &'static Schema {
            &SCHEMA
        }
    }

    #[derive(Debug)]
    struct Detail;
    impl SourceValueDetail for Detail {
        fn label(&self) -> String {
            "test".into()
        }
    }

    struct TestSource {
        allow_unknown_paths: bool,
        values: Vec<(ConfigPath, Value)>,
    }

    impl ConfigSource for TestSource {
        type Error = String;
        type Detail = Detail;
        fn settings(&self) -> ConfigSourceSettings {
            ConfigSourceSettings::builder()
                .allow_unknown_paths(self.allow_unknown_paths)
                .build()
        }
        fn load(&self, _schema: &'static Schema) -> Result<Vec<FieldCandidate<Detail>>, String> {
            Ok(self
                .values
                .iter()
                .map(|(path, value)| FieldCandidate::new(path.clone(), value.clone(), Detail))
                .collect())
        }
    }

    fn source(values: Vec<(ConfigPath, Value)>) -> BoundSource<NoPath, String> {
        source_with(false, values)
    }

    fn source_with(
        allow_unknown_paths: bool,
        values: Vec<(ConfigPath, Value)>,
    ) -> BoundSource<NoPath, String> {
        BoundSource::new_anonymous(
            TestSource {
                allow_unknown_paths,
                values,
            },
            SourceRegistration::new(|error| error, |_, _| NoPath),
        )
    }

    fn path(segments: &[&'static str]) -> ConfigPath {
        ConfigPath::new(segments.iter().map(|s| ConfigPathSegment::new(s)).collect())
    }

    // r[verify load.precedence]
    // r[verify load.decode]
    #[test]
    fn later_sources_shadow_earlier_ones() {
        let sources = vec![
            source(vec![
                (path(&["name"]), Value::from("base")),
                (path(&["jwt", "ttl_seconds"]), Value::from(60u32)),
            ]),
            source(vec![(path(&["jwt", "ttl_seconds"]), Value::from(120u32))]),
        ];
        let loaded = load_config_from_sources::<App, NoPath, String>(sources).unwrap();
        assert_eq!(
            loaded.config(),
            &App {
                name: "base".into(),
                jwt: Jwt {
                    ttl_seconds: 120,
                    issuer: None,
                },
            }
        );
    }

    // r[verify load.unknown-paths]
    #[test]
    fn unknown_paths_are_reported() {
        let sources = vec![source(vec![
            (path(&["name"]), Value::from("app")),
            (path(&["jwt", "ttl_seconds"]), Value::from(1u32)),
            (path(&["jwt", "issuer", "nope"]), Value::from("nope")),
        ])];
        let Err(ConfigLoadError::Violations(violations)) =
            load_config_from_sources::<App, NoPath, String>(sources)
        else {
            panic!("expected violations");
        };
        assert_eq!(violations.unknown_paths().len(), 1);
    }

    // r[verify load.unknown-paths.allow]
    // r[verify provenance.unset]
    #[test]
    fn unknown_paths_are_dropped_when_allowed() {
        let sources = vec![source_with(
            true,
            vec![
                (path(&["name"]), Value::from("app")),
                (path(&["jwt", "ttl_seconds"]), Value::from(1u32)),
                (path(&["jwt", "nope"]), Value::from("nope")),
            ],
        )];
        let loaded = load_config_from_sources::<App, NoPath, String>(sources).unwrap();
        assert_eq!(loaded.config().jwt.ttl_seconds, 1);
        assert!(loaded.source(&path(&["jwt", "nope"])).is_none());
    }

    // r[verify load.conflicts]
    #[test]
    fn values_provided_twice_by_one_source_conflict() {
        let sources = vec![source(vec![
            (path(&["name"]), Value::from("a")),
            (path(&["name"]), Value::from("b")),
            (path(&["jwt", "ttl_seconds"]), Value::from(1u32)),
        ])];
        let Err(ConfigLoadError::Violations(violations)) =
            load_config_from_sources::<App, NoPath, String>(sources)
        else {
            panic!("expected violations");
        };
        let [conflict] = violations.conflicts() else {
            panic!("expected one conflict");
        };
        assert_eq!(conflict.path, path(&["name"]));
        assert_eq!(conflict.contenders.len(), 2);
    }

    // r[verify load.decode.strings]
    #[test]
    fn string_values_are_parsed_into_field_types() {
        let sources = vec![source(vec![
            (path(&["name"]), Value::from("app")),
            (path(&["jwt", "ttl_seconds"]), Value::from("60")),
        ])];
        let loaded = load_config_from_sources::<App, NoPath, String>(sources).unwrap();
        assert_eq!(loaded.config().jwt.ttl_seconds, 60);
    }

    // r[verify load.decode]
    #[test]
    fn missing_required_fields_fail_to_decode() {
        let sources = vec![source(vec![(path(&["name"]), Value::from("app"))])];
        let Err(ConfigLoadError::Violations(violations)) =
            load_config_from_sources::<App, NoPath, String>(sources)
        else {
            panic!("expected violations");
        };
        assert_eq!(violations.decode_errors().len(), 1);
    }
}

# core

The core of ordna: config models, schemas, sources, the builder, loading and provenance.

- **`ConfigModel`**: a config structure, usually implemented with `#[config]` or
  `config_tree!` (re-exported from [macros](../macros)).
- **`Schema`**: the fields of a model, as `ConfigPath`s such as `db.password`, and each
  field's layer, public or secret.
- **`ConfigSource`**: provides `FieldCandidate`s, values for config paths, along with a
  detail of where each came from. See the source crates for implementations.
- **`ConfigBuilder`**: loads a model from sources applied in order, later ones shadowing
  earlier ones.
- **`LoadedConfig`**: the loaded model, with the provenance of every set field: the
  winning source and the ones it shadowed.
- **`ConfigLoadError`**: either a source failing to load, or all violations found
  (unknown paths, conflicts, layer mismatches and decode errors).

## Sources and provenance

Registering a source type maps its errors into your error type, and the detail of each
value into your provenance type:

```rust
let env = App::register_source(Error::Env, |path, detail: EnvVarDetail| Origin::Env {
    var: detail.name,
    path,
});

let loaded = ConfigBuilder::<App, _, _>::new()
    .source_named("env", env, EnvSource::with_prefix("APP"))
    .load_with_meta()?;

let port = loaded.source(&ConfigPath::parse("port")?).unwrap();
println!("{:?} shadowed {:?}", port.winner(), port.shadowed());
```

## Secrets

Fields marked `#[config(secret)]` (or `#[facet(sensitive)]`) are secret, along with every
field nested in them. A source may only set fields of its own layer: secret fields only
from secret sources, public fields only from public sources. Values in violations are
redacted when either side is secret.

The package is named `core`, so depend on it as `ordna-core`:

```toml
ordna-core = { package = "core", path = "../core" }
```

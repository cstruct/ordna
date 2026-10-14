# ordna

Layered configuration for Rust: declare a config structure, load it from files and
environment variables, and find out where every value came from.

> **Note on LLM use:** large parts of this project were written with an LLM, under human
> direction. The design and review are human, and it isn't entirely LLM-written.

- **Typed models** declared as normal structs or as one nested declaration, built on
  [facet](https://facet.rs).
- **Layered sources** applied in order, later ones overriding earlier ones.
- **Secrets** kept apart: secret fields only load from secret sources, the environment is
  never one, and secret values are redacted in errors.
- **Provenance** for every field: the source that won and the ones it shadowed.
- **All problems at once**: unknown keys, conflicts, secrets in the wrong place and decode
  errors are collected and reported together.

## Example

```rust
use std::path::PathBuf;

use ordna_core::{ConfigBuilder, ConfigLoadError, ConfigPath, SourceFieldPath, config_tree};
use source_env::{EnvSource, EnvSourceError, EnvVarDetail};
use source_file::{FileSourceError, FileValueDetail};
use source_styx::{DeserializeError, styx_file};

config_tree! {
    #[derive(Debug)]
    pub struct App {
        pub name: String,
        pub port: u16,
        #[config(path = "db")]
        pub database: pub struct Database {
            pub host: String,
            #[config(secret)]
            pub password: String,
        },
    }
}

/// Where a value came from.
#[derive(Debug)]
enum Origin {
    Env { var: String, path: ConfigPath },
    File { file: PathBuf, path: ConfigPath },
}

impl SourceFieldPath for Origin {
    fn field_path(&self) -> &ConfigPath {
        match self {
            Origin::Env { path, .. } | Origin::File { path, .. } => path,
        }
    }
}

#[derive(Debug)]
enum Error {
    Env(EnvSourceError),
    File(FileSourceError<DeserializeError>),
}

fn main() -> Result<(), ConfigLoadError<Error>> {
    let env = App::register_source(Error::Env, |path, detail: EnvVarDetail| Origin::Env {
        var: detail.name,
        path,
    });
    let file = || {
        App::register_source(Error::File, |path, detail: FileValueDetail| Origin::File {
            file: detail.file,
            path,
        })
    };

    let loaded = ConfigBuilder::<App, _, _>::new()
        .source_named("defaults", file(), styx_file("config.styx"))
        .source_named("secrets", file(), styx_file("secrets.styx").optional().secrets())
        .source_named("env", env, EnvSource::with_prefix("APP"))
        .load_with_meta()?;

    println!("{:?}", loaded.config());
    let port = loaded.source(&ConfigPath::parse("port").unwrap()).unwrap();
    println!("port came from {:?}", port.winner());
    Ok(())
}
```

With `config.styx`:

```styx
name app
port 8080
db {
  host localhost
}
```

`secrets.styx` holds `db.password`, and `APP__PORT=9090` overrides the port.

## Crates

| Crate | |
|---|---|
| [core](crates/core) | Config models, schemas, the builder, loading and provenance |
| [macros](crates/macros) | `#[config]` and `config_tree!`, re-exported by `core` |
| [source-env](crates/source-env) | Environment variables |
| [source-file](crates/source-file) | Generic file source for any `FileFormat` |
| [source-styx](crates/source-styx) | [Styx](https://github.com/bearcove/styx) files |
| [source-json](crates/source-json) | JSON files |
| [source-toml](crates/source-toml) | TOML files |
| [source-yaml](crates/source-yaml) | YAML files |
| [integration-tests](crates/integration-tests) | End-to-end tests across the crates |

The core package is named `core`, which clashes with Rust's built-in `core` crate, so
depend on it under another name. The macros expect `ordna-core`:

```toml
[dependencies]
ordna-core = { package = "core", path = "crates/core" }
# Needed when using `#[config(path = "...")]`, see crates/macros.
facet = "0.46.5"
```

## Development

```bash
cargo test --workspace
```

The behaviour is specified in [docs/spec](docs/spec) and tracked with
[tracey](https://github.com/bearcove/tracey): code references requirements with
`r[impl ...]` and tests with `r[verify ...]`.

```bash
tracey query status
```

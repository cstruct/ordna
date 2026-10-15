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
use ordna_source_env::{EnvSource, EnvSourceError, EnvVarDetail};
use ordna_source_file::{FileSourceError, FileValueDetail};
use ordna_source_styx::{DeserializeError, styx_file};

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
| [ordna-core](crates/core) | Config models, schemas, the builder, loading and provenance |
| [ordna-macros](crates/macros) | `#[config]` and `config_tree!`, re-exported by `ordna-core` |
| [ordna-source-env](crates/source-env) | Environment variables |
| [ordna-source-file](crates/source-file) | Generic file source for any `FileFormat` |
| [ordna-source-styx](crates/source-styx) | [Styx](https://github.com/bearcove/styx) files |
| [ordna-source-json](crates/source-json) | JSON files |
| [ordna-source-toml](crates/source-toml) | TOML files |
| [ordna-source-yaml](crates/source-yaml) | YAML files |
| [integration-tests](crates/integration-tests) | End-to-end tests across the crates |

Depend on `ordna-core` plus the source crates you need:

```toml
[dependencies]
ordna-core = "0.1"
ordna-source-env = "0.1"
ordna-source-styx = "0.1"
# Needed when using `#[config(path = "...")]`, see crates/macros.
facet = "0.46.5"
```

## Development

[mise](https://mise.jdx.dev) installs the pinned Rust toolchain and tracey from
[mise.toml](mise.toml).

```bash
mise install
cargo test --workspace
```

The behaviour is specified in [docs/spec](docs/spec) and tracked with
[tracey](https://github.com/bearcove/tracey): code references requirements with
`r[impl ...]` and tests with `r[verify ...]`.

```bash
tracey query status
```

## Releasing

The crates share the workspace version and are released together. Bump it, along with the
crates' requirements on each other, with
[cargo-edit](https://github.com/killercup/cargo-edit), then tag the commit:

```bash
cargo set-version --workspace 0.2.0
```

Pushing a `v0.2.0` tag runs the [release workflow](.github/workflows/release.yml): it checks
the tag against the version, runs CI, including
[cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks) against the last
release, then publishes the crates to crates.io and creates a GitHub release.

A new crate's first version has to be published by hand (`cargo publish -p <crate>`), and
its trusted publisher set up on crates.io, before a release can publish it: crates.io only
allows trusted publishing for crates that exist. The release skips crates already published
at its version.

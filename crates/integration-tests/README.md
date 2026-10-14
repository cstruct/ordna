# integration-tests

End-to-end tests of the ordna crates, not published.

| Test | Covers |
|---|---|
| [env_styx.rs](tests/env_styx.rs) | Public and secret Styx files with environment overrides: values, provenance, secrets rules, unknown paths and source errors |
| [formats.rs](tests/formats.rs) | The same config from Styx, JSON, TOML and YAML, and from a mix of formats |
| [process_env.rs](tests/process_env.rs) | Reading the real process environment, alone in its binary since it sets variables |
| [macro_errors.rs](tests/macro_errors.rs) | Compile errors of the macros, with [trybuild](https://crates.io/crates/trybuild) cases in [tests/ui](tests/ui) |

The shared config model, provenance type and source setup live in
[tests/common](tests/common/mod.rs).

```bash
cargo test -p integration-tests
```

After changing a macro error message, update the expected output with:

```bash
TRYBUILD=overwrite cargo test -p integration-tests --test macro_errors
```

# ordna-source-toml

Loads ordna config values from TOML files, using [facet-toml](https://crates.io/crates/facet-toml).

```rust
use ordna_source_toml::toml_file;

let defaults = toml_file("config.toml");
let secrets = toml_file("secrets.toml").optional().secrets();
```

```toml
name = "app"

[jwt]
ttl_seconds = 60
```

`toml_file` returns a `TomlSource`, a [`FileSource`](../source-file) with the `Toml`
format, so all file source behaviour applies: unknown keys are reported, missing files
fail unless optional, and a file is either public or secrets only.

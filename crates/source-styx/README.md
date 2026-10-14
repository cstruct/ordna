# ordna-source-styx

Loads ordna config values from [Styx](https://github.com/bearcove/styx) files, using [facet-styx](https://crates.io/crates/facet-styx).

```rust
use ordna_source_styx::styx_file;

let defaults = styx_file("config.styx");
let secrets = styx_file("secrets.styx").optional().secrets();
```

```styx
name app
jwt {
  ttl_seconds 60
}
```

Styx scalars are untyped, so every scalar is passed on as a string and parsed into the
field's type when the config is decoded.

`styx_file` returns a `StyxSource`, a [`FileSource`](../source-file) with the `Styx`
format, so all file source behaviour applies: unknown keys are reported, missing files
fail unless optional, and a file is either public or secrets only.

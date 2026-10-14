# source-json

Loads ordna config values from JSON files, using [facet-json](https://crates.io/crates/facet-json).

```rust
use source_json::json_file;

let defaults = json_file("config.json");
let secrets = json_file("secrets.json").optional().secrets();
```

```json
{
  "name": "app",
  "jwt": { "ttl_seconds": 60 }
}
```

`json_file` returns a `JsonSource`, a [`FileSource`](../source-file) with the `Json`
format, so all file source behaviour applies: unknown keys are reported, missing files
fail unless optional, and a file is either public or secrets only.

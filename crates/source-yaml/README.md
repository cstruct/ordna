# source-yaml

Loads ordna config values from YAML files, using [facet-yaml](https://crates.io/crates/facet-yaml).

```rust
use source_yaml::yaml_file;

let defaults = yaml_file("config.yaml");
let secrets = yaml_file("secrets.yaml").optional().secrets();
```

```yaml
name: app
jwt:
  ttl_seconds: 60
```

`yaml_file` returns a `YamlSource`, a [`FileSource`](../source-file) with the `Yaml`
format, so all file source behaviour applies: unknown keys are reported, missing files
fail unless optional, and a file is either public or secrets only.

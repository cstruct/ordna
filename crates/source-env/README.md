# source-env

Loads ordna config values from environment variables.

```rust
use source_env::EnvSource;

// Reads APP__NAME, APP__JWT__TTL_SECONDS, ...
let env = EnvSource::with_prefix("APP");
```

- Each field maps to the prefix and its config path segments, joined by the separator
  (`__` by default, see `.separator()`) and uppercased: `jwt.ttl_seconds` becomes
  `APP__JWT__TTL_SECONDS`. Names match case-insensitively.
- Values are passed on as strings and parsed into the field's type when decoding.
- With a prefix, prefixed variables that match no field are reported as unknown paths,
  unless `.allow_unknown_paths(true)` is set. Without a prefix they're ignored.
- With a prefix, variables can also set fields under map keys:
  `APP__BACKENDS__EU_WEST__REGION` sets `backends.eu_west.region`. Map keys from variable
  names are lowercased.
- The environment is always a public source: it can't set secret fields.
- `EnvSource::from_vars(...)` reads given variables instead of the process environment.

# ordna-macros

Macros for declaring ordna config models. Use them through `ordna_core`, which re-exports
them.

## `#[config]`

Turns a normal struct into a config model: derives `Facet` (don't derive it yourself) and
implements `ConfigModel`.

```rust
use ordna_core::config;

#[config]
#[derive(Debug)]
pub struct App {
    pub name: String,
    #[config(path = "token")]
    pub jwt: Jwt,
}

#[config]
#[derive(Debug)]
pub struct Jwt {
    #[config(secret)]
    pub signing_key: String,
    pub ttl_seconds: u32,
}
```

On an enum, `#[config]` derives `Facet` (adding `#[repr(C)]` unless it has a `repr`) and
handles the field attributes of its struct variants. Tag it with `#[facet(tag = "...")]`
to have its variants' fields walked as config fields:

```rust
#[config]
#[facet(tag = "type", rename_all = "snake_case")]
pub enum Backend {
    S3 {
        bucket: String,
        #[config(secret)]
        secret_access_key: String,
    },
    Swift {
        container: String,
        #[config(secret)]
        password: String,
    },
}
```

facet only supports struct and unit variants for internally tagged enums, so tuple
variants are a compile error.

## `config_tree!`

Declares nested structs and enums inline, as field types or inside their generic
arguments. Every declaration is expanded as if it had `#[config]`, and `#[derive(...)]`
attributes are passed on to nested declarations.

```rust
use ordna_core::config_tree;

config_tree! {
    #[derive(Debug)]
    pub struct App {
        pub name: String,
        #[config(path = "token")]
        pub jwt: pub struct Jwt {
            #[config(secret)]
            pub signing_key: String,
            pub ttl_seconds: u32,
        },
        pub mappings: Vec<pub struct Mapping {
            pub path_prefix: String,
        }>,
        pub backends: HashMap<String, #[facet(tag = "type")] pub enum Backend {
            S3 {
                bucket: String,
                #[config(secret)]
                secret_access_key: String,
            },
        }>,
    }
}
```

## Field attributes

- `#[config(path = "segment")]` uses `segment` as the field's config path segment instead
  of its name. It must be a single valid segment.
- `#[config(secret)]` marks the field, and every field nested in it, as secret.

Invalid paths, duplicate paths, unknown attributes and generic structs are compile errors.

Crates using `#[config(path = "...")]` or facet attributes such as `tag` must depend on
`facet` directly: facet expands those attributes through a macro that always refers to
`::facet`.

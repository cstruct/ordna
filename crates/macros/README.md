# macros

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

## `config_tree!`

Declares nested structs inline. Every struct is expanded as if it had `#[config]`, and
`#[derive(...)]` attributes are passed on to nested structs.

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
    }
}
```

## Field attributes

- `#[config(path = "segment")]` uses `segment` as the field's config path segment instead
  of its name. It must be a single valid segment.
- `#[config(secret)]` marks the field, and every field nested in it, as secret.

Invalid paths, duplicate paths, unknown attributes and generic structs are compile errors.

Crates using `#[config(path = "...")]` must depend on `facet` directly: facet expands the
`rename` it maps to through a macro that always refers to `::facet`.

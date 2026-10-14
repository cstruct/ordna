# Macros

Config models are declared with the `#[config]` attribute on normal structs, or with the
`config_tree!` macro for nested declarations.

## Declaring models

r[macro.attribute]
`#[config]` on a struct with named fields MUST derive `Facet` for it and implement
`ConfigModel`.

r[macro.tree]
`config_tree!` MUST accept struct declarations as field types and expand every declared
struct as if it had `#[config]`, with nested structs as their own types.

r[macro.tree.derives]
`config_tree!` MUST pass a struct's `#[derive(...)]` attributes on to the structs and enums
nested in it.

r[macro.tree.enums]
`config_tree!` MUST accept enum declarations as field types and expand them as if they had
`#[config]`.

r[macro.tree.generics]
`config_tree!` MUST accept struct and enum declarations inside the generic arguments of a
field type, such as `Vec<struct Mapping { .. }>` or `HashMap<String, enum Backend { .. }>`.

r[macro.enum]
`#[config]` on an enum MUST derive `Facet` for it, giving it a `#[repr(C)]` representation
unless it has one, and handle the field attributes of its struct variants.

r[macro.raw-idents]
A field named with a raw identifier MUST use the name without the `r#` prefix as its
config path segment.

## Field attributes

r[macro.path]
`#[config(path = "segment")]` on a field MUST use `segment` as the field's config path
segment instead of its name.

r[macro.secret]
`#[config(secret)]` on a field MUST mark the field as secret.

## Errors

r[macro.error.path]
An overridden path that is not a single valid segment MUST be a compile error.

r[macro.error.duplicate-path]
Two fields of a struct with the same config path segment MUST be a compile error.

r[macro.error.attributes]
Unknown `config` attributes, duplicate `config` attributes, `#[config(...)]` on structs and
`#[facet(rename)]` on fields MUST be compile errors.

r[macro.error.shape]
Generic structs and enums, structs without named fields and enum variants that are neither
struct nor unit variants MUST be compile errors.

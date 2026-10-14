//! Macros for declaring config structures.
//!
//! - [`macro@config`] turns a normal struct into a config struct.
//! - [`config_tree!`] declares a config structure with nested structs inline.
//!
//! In both, field attributes configure fields:
//!
//! - `#[config(path = "segment")]` overrides the field's config path segment (by default
//!   the field name).
//! - `#[config(secret)]` marks the field as secret, along with every field nested in it.
//!   Secret fields can only be loaded from secret sources.
//!
//! Crates using `#[config(path = "...")]` must depend on `facet` directly: facet expands
//! `rename` (which `path` maps to) through its attribute grammar, which always refers to
//! `::facet` regardless of the `#[facet(crate = ...)]` override the macros emit.

use std::collections::BTreeMap;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::{
    Attribute, Error, Fields, Ident, ItemStruct, LitStr, Result, Token, Type, Visibility, braced,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

// r[impl macro.attribute]
/// Turns a struct into a config struct.
///
/// Derives `Facet` (don't derive it yourself) and implements `ConfigModel`. Nested
/// structs used as fields should be `#[config]` structs as well, so their fields can
/// override paths too.
///
/// ```ignore
/// #[config]
/// struct App {
///     name: String,
///     #[config(path = "token")]
///     jwt: Jwt,
/// }
///
/// #[config]
/// struct Jwt {
///     ttl_seconds: u32,
/// }
/// ```
#[proc_macro_attribute]
pub fn config(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = TokenStream2::from(args);
    if !args.is_empty() {
        return Error::new_spanned(args, "`#[config]` takes no arguments")
            .into_compile_error()
            .into();
    }
    let item = parse_macro_input!(input as ItemStruct);
    ConfigStruct::from_item(item)
        .and_then(|config| config.expand(&[]))
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

// r[impl macro.tree]
/// Declares a config structure, with nested structs declared inline as field types.
///
/// Every struct becomes a [`macro@config`] struct. `#[derive(...)]` attributes are
/// passed on to nested structs.
///
/// ```ignore
/// config_tree! {
///     #[derive(Debug)]
///     pub struct App {
///         name: String,
///         #[config(path = "token")]
///         jwt: pub struct Jwt {
///             ttl_seconds: u32,
///         },
///     }
/// }
/// ```
#[proc_macro]
pub fn config_tree(input: TokenStream) -> TokenStream {
    let config = parse_macro_input!(input as ConfigStruct);
    config
        .expand(&[])
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

struct ConfigStruct {
    attrs: Vec<Attribute>,
    vis: Visibility,
    struct_token: Token![struct],
    ident: Ident,
    fields: Vec<ConfigField>,
}

struct ConfigField {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
    ty: FieldType,
}

enum FieldType {
    Type(Type),
    Nested(Box<ConfigStruct>),
}

impl ConfigStruct {
    // r[impl macro.error.shape]
    fn from_item(item: ItemStruct) -> Result<Self> {
        if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
            return Err(Error::new_spanned(
                item.generics,
                "config structs can't be generic",
            ));
        }
        let Fields::Named(fields) = item.fields else {
            return Err(Error::new_spanned(
                item.fields,
                "config structs must have named fields",
            ));
        };
        Ok(Self {
            attrs: item.attrs,
            vis: item.vis,
            struct_token: item.struct_token,
            ident: item.ident,
            fields: fields
                .named
                .into_iter()
                .map(|field| ConfigField {
                    attrs: field.attrs,
                    vis: field.vis,
                    ident: field.ident.expect("named fields have identifiers"),
                    ty: FieldType::Type(field.ty),
                })
                .collect(),
        })
    }

    /// Expands this struct, and any nested structs, into `#[config]` structs.
    /// `inherited` are attributes passed down from parent structs.
    fn expand(&self, inherited: &[Attribute]) -> Result<TokenStream2> {
        let Self {
            attrs,
            vis,
            struct_token,
            ident,
            fields,
        } = self;

        let mut attrs = inherited.iter().chain(attrs).cloned().collect::<Vec<_>>();
        reject_config_attrs(&attrs)?;
        dedup_attrs(&mut attrs);
        // r[impl macro.tree.derives]
        let derives = attrs
            .iter()
            .filter(|attr| attr.path().is_ident("derive"))
            .cloned()
            .collect::<Vec<_>>();

        let mut nested = Vec::new();
        let mut segments = BTreeMap::<String, Span>::new();
        let mut field_tokens = Vec::new();
        for field in fields {
            let (field_attrs, segment) = field_attrs(field)?;
            let (name, span) = match segment {
                Some(segment) => (segment.value(), segment.span()),
                None => (unraw(&field.ident), field.ident.span()),
            };
            // r[impl macro.error.duplicate-path]
            if let Some(previous) = segments.insert(name.clone(), span) {
                let mut error = Error::new(span, format!("duplicate config path `{name}`"));
                error.combine(Error::new(previous, "previously used here"));
                return Err(error);
            }

            let ty = match &field.ty {
                FieldType::Type(ty) => ty.to_token_stream(),
                // r[impl macro.tree]
                FieldType::Nested(config) => {
                    nested.push(config.expand(&derives)?);
                    config.ident.to_token_stream()
                }
            };
            let ConfigField {
                vis, ident: name, ..
            } = field;
            field_tokens.push(quote! {
                #(#field_attrs)*
                #vis #name: #ty
            });
        }

        Ok(quote! {
            #[derive(::ordna_core::__private::facet::Facet)]
            #[facet(crate = ::ordna_core::__private::facet)]
            #(#attrs)*
            #vis #struct_token #ident {
                #(#field_tokens,)*
            }

            impl<P, E> ::ordna_core::ConfigModel<'static, P, E> for #ident
            where
                P: ::ordna_core::SourceFieldPath,
                E: ::core::marker::Send + ::core::marker::Sync + 'static,
            {
                fn schema() -> &'static ::ordna_core::Schema {
                    static SCHEMA: ::ordna_core::Schema = ::ordna_core::Schema::new::<#ident>();
                    &SCHEMA
                }
            }

            #(#nested)*
        })
    }
}

// r[impl macro.error.attributes]
/// Field attributes with `#[config(...)]` replaced by the matching facet attributes, and
/// the overridden path segment, if any.
fn field_attrs(field: &ConfigField) -> Result<(Vec<Attribute>, Option<LitStr>)> {
    let mut attrs = Vec::new();
    let mut segment = None::<LitStr>;
    let mut secret = false;
    for attr in &field.attrs {
        if !attr.path().is_ident("config") {
            if attr.path().is_ident("facet") && has_facet_rename(attr) {
                return Err(Error::new_spanned(
                    attr,
                    "use `#[config(path = \"...\")]` instead of `#[facet(rename = \"...\")]`",
                ));
            }
            attrs.push(attr.clone());
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("path") {
                if segment.is_some() {
                    return Err(meta.error("duplicate `path`"));
                }
                let value: LitStr = meta.value()?.parse()?;
                validate_segment(&value)?;
                segment = Some(value);
                Ok(())
            } else if meta.path.is_ident("secret") {
                if secret {
                    return Err(meta.error("duplicate `secret`"));
                }
                secret = true;
                Ok(())
            } else {
                Err(meta.error("unknown config attribute, expected `path` or `secret`"))
            }
        })?;
    }
    if let Some(segment) = &segment {
        // r[impl macro.path]
        attrs.push(syn::parse_quote!(#[facet(rename = #segment)]));
    }
    if secret {
        // r[impl macro.secret]
        attrs.push(syn::parse_quote!(#[facet(sensitive)]));
    }
    Ok((attrs, segment))
}

fn has_facet_rename(attr: &Attribute) -> bool {
    attr.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)
        .is_ok_and(|metas| metas.iter().any(|meta| meta.path().is_ident("rename")))
}

// r[impl macro.error.attributes]
fn reject_config_attrs(attrs: &[Attribute]) -> Result<()> {
    match attrs.iter().find(|attr| attr.path().is_ident("config")) {
        Some(attr) => Err(Error::new_spanned(
            attr,
            "`#[config(...)]` is only supported on fields",
        )),
        None => Ok(()),
    }
}

/// Removes repeated attributes, which inheriting derives from parents can produce.
fn dedup_attrs(attrs: &mut Vec<Attribute>) {
    let mut seen = Vec::<String>::new();
    attrs.retain(|attr| {
        let key = attr.to_token_stream().to_string();
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
}

// r[impl macro.error.path]
/// Mirrors the rules of `ConfigPathSegment`, so invalid paths fail at compile time.
fn validate_segment(segment: &LitStr) -> Result<()> {
    let value = segment.value();
    let error = |message| Err(Error::new(segment.span(), message));
    if value.is_empty() {
        return error("config path segments must not be empty");
    }
    if value.contains('.') {
        return error("config paths override a single segment and must not contain `.`");
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return error(
            "config path segments may only contain ASCII alphanumerics, underscores and hyphens",
        );
    }
    Ok(())
}

// r[impl macro.raw-idents]
fn unraw(ident: &Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").map(str::to_string).unwrap_or(name)
}

impl Parse for ConfigStruct {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis = input.parse()?;
        let struct_token = input.parse()?;
        let ident = input.parse()?;
        let content;
        braced!(content in input);
        let fields = Punctuated::<ConfigField, Token![,]>::parse_terminated(&content)?;
        Ok(Self {
            attrs,
            vis,
            struct_token,
            ident,
            fields: fields.into_iter().collect(),
        })
    }
}

impl Parse for ConfigField {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis = input.parse()?;
        let ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty = if is_nested_struct(input) {
            FieldType::Nested(Box::new(input.parse()?))
        } else {
            FieldType::Type(input.parse()?)
        };
        Ok(Self {
            attrs,
            vis,
            ident,
            ty,
        })
    }
}

/// Whether the input continues with `[attributes] [visibility] struct`.
fn is_nested_struct(input: ParseStream) -> bool {
    let fork = input.fork();
    fork.call(Attribute::parse_outer).is_ok()
        && fork.parse::<Visibility>().is_ok()
        && fork.peek(Token![struct])
}

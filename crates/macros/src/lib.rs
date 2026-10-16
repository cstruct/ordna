//! Macros for declaring config structures.
//!
//! - [`macro@config`] turns a normal struct into a config struct, or an enum into one
//!   usable in config structs.
//! - [`config_tree!`] declares a config structure with nested structs inline.
//! - [`config_contexts!`] declares a set of config models, contexts, loaded from the same
//!   sources.
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
    Attribute, Error, Fields, Ident, Item, ItemEnum, ItemStruct, LitStr, Result, Token, Type,
    Visibility, braced,
    parse::{Parse, ParseStream, discouraged::Speculative},
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
///
/// On an enum, derives `Facet` and handles the field attributes of its struct variants.
/// Tag the enum with `#[facet(tag = "...")]` to have its variants' fields walked as config
/// fields; facet only supports struct and unit variants for internally tagged enums.
///
/// ```ignore
/// #[config]
/// #[facet(tag = "type", rename_all = "snake_case")]
/// #[repr(C)]
/// enum Backend {
///     S3 {
///         bucket: String,
///         #[config(secret)]
///         secret_access_key: String,
///     },
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
    let result = match parse_macro_input!(input as Item) {
        Item::Struct(item) => ConfigStruct::from_item(item).and_then(|config| config.expand(&[])),
        Item::Enum(item) => expand_enum(item),
        item => Err(Error::new_spanned(
            item,
            "`#[config]` is only supported on structs and enums",
        )),
    };
    result.unwrap_or_else(Error::into_compile_error).into()
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

// r[impl macro.contexts]
/// Declares a set of config contexts: config models loaded from the same sources, each
/// entrypoint loading only the ones it needs.
///
/// Sources are checked against the fields of every context in the set, so a value for a
/// field of any context is known, even when loading others. Contexts may share fields,
/// with different types or requirements, such as `db` required in one and optional in
/// another. A field that is secret in any context is secret in all of them.
///
/// ```ignore
/// config_contexts!(pub AppContexts = [Http, Db, DbOpt, Mailer]);
///
/// let (http, db) = ConfigBuilder::<AppContexts, _, _>::new()
///     .source_named("env", AppContexts::register_source(..), EnvSource::with_prefix("APP"))
///     .load_contexts::<(Http, Db)>()?;
/// ```
#[proc_macro]
pub fn config_contexts(input: TokenStream) -> TokenStream {
    parse_macro_input!(input as ConfigContexts).expand().into()
}

struct ConfigContexts {
    attrs: Vec<Attribute>,
    vis: Visibility,
    ident: Ident,
    contexts: Vec<Type>,
}

impl Parse for ConfigContexts {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis = input.parse()?;
        let ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let content;
        syn::bracketed!(content in input);
        let contexts = Punctuated::<Type, Token![,]>::parse_terminated(&content)?;
        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
        }
        Ok(Self {
            attrs,
            vis,
            ident,
            contexts: contexts.into_iter().collect(),
        })
    }
}

impl ConfigContexts {
    fn expand(&self) -> TokenStream2 {
        let Self {
            attrs,
            vis,
            ident,
            contexts,
        } = self;
        quote! {
            #(#attrs)*
            #vis enum #ident {}

            impl ::ordna_core::ConfigSchema for #ident {
                fn schema() -> &'static ::ordna_core::Schema {
                    static SHAPES: &[&::ordna_core::__private::facet::Shape] = &[
                        #(<#contexts as ::ordna_core::__private::facet::Facet<'static>>::SHAPE,)*
                    ];
                    static SCHEMA: ::ordna_core::Schema = ::ordna_core::Schema::union(SHAPES);
                    &SCHEMA
                }
            }

            impl #ident {
                /// Registers source type `S`, mapping its errors into `E` and its value
                /// details into the provenance type `P`. See
                /// [`SourceRegistration::new`](::ordna_core::SourceRegistration::new).
                #vis fn register_source<S, P, E>(
                    error_mapper: impl ::core::ops::Fn(S::Error) -> E
                        + ::core::marker::Send + ::core::marker::Sync + 'static,
                    origin_mapper: impl ::core::ops::Fn(::ordna_core::ConfigPath, S::Detail) -> P
                        + ::core::marker::Send + ::core::marker::Sync + 'static,
                ) -> ::ordna_core::SourceRegistration<S, P, E>
                where
                    S: ::ordna_core::ConfigSource,
                {
                    ::ordna_core::SourceRegistration::new(error_mapper, origin_mapper)
                }
            }

            #(impl ::ordna_core::ConfigContext<#ident> for #contexts {})*
        }
    }
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

#[allow(
    clippy::large_enum_variant,
    reason = "only lives while expanding a macro, where size doesn't matter"
)]
enum FieldType {
    Type(Type),
    /// A struct declared as the type.
    Struct(Box<ConfigStruct>),
    /// An enum declared as the type.
    Enum(Box<ItemEnum>),
    /// A generic type with declarations in its arguments, such as
    /// `Vec<struct Mapping { .. }>`.
    Generic {
        path: TokenStream2,
        args: Vec<FieldType>,
    },
}

impl FieldType {
    /// The type's tokens, expanding the structs and enums declared in it into `nested`.
    fn expand(
        &self,
        derives: &[Attribute],
        nested: &mut Vec<TokenStream2>,
    ) -> Result<TokenStream2> {
        Ok(match self {
            FieldType::Type(ty) => ty.to_token_stream(),
            // r[impl macro.tree]
            FieldType::Struct(config) => {
                nested.push(config.expand(derives)?);
                config.ident.to_token_stream()
            }
            // r[impl macro.tree.enums]
            FieldType::Enum(item) => {
                let mut item = (**item).clone();
                item.attrs = derives.iter().chain(&item.attrs).cloned().collect();
                dedup_attrs(&mut item.attrs);
                let ident = item.ident.to_token_stream();
                nested.push(expand_enum(item)?);
                ident
            }
            // r[impl macro.tree.generics]
            FieldType::Generic { path, args } => {
                let args = args
                    .iter()
                    .map(|arg| arg.expand(derives, nested))
                    .collect::<Result<Vec<_>>>()?;
                quote! { #path<#(#args),*> }
            }
        })
    }
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
            let (field_attrs, segment) = field_attrs(&field.attrs)?;
            check_duplicate(&mut segments, &field.ident, segment.as_ref())?;

            let ty = field.ty.expand(&derives, &mut nested)?;
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

            impl ::ordna_core::ConfigSchema for #ident {
                fn schema() -> &'static ::ordna_core::Schema {
                    static SCHEMA: ::ordna_core::Schema = ::ordna_core::Schema::new::<#ident>();
                    &SCHEMA
                }
            }

            impl<P, E> ::ordna_core::ConfigModel<'static, P, E> for #ident
            where
                P: ::ordna_core::SourceFieldPath,
                E: ::core::marker::Send + ::core::marker::Sync + 'static,
            {
            }

            #(#nested)*
        })
    }
}

// r[impl macro.enum]
fn expand_enum(mut item: ItemEnum) -> Result<TokenStream2> {
    if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            item.generics,
            "config enums can't be generic",
        ));
    }
    reject_config_attrs(&item.attrs)?;
    for variant in &mut item.variants {
        reject_config_attrs(&variant.attrs)?;
        match &mut variant.fields {
            Fields::Named(fields) => {
                let mut segments = BTreeMap::new();
                for field in &mut fields.named {
                    let (attrs, segment) = field_attrs(&field.attrs)?;
                    let ident = field.ident.as_ref().expect("named fields have identifiers");
                    check_duplicate(&mut segments, ident, segment.as_ref())?;
                    field.attrs = attrs;
                }
            }
            Fields::Unit => {}
            Fields::Unnamed(fields) => {
                return Err(Error::new_spanned(
                    fields,
                    "config enum variants must be struct or unit variants",
                ));
            }
        }
    }
    // facet requires an explicit representation for enums.
    let repr = (!item.attrs.iter().any(|attr| attr.path().is_ident("repr")))
        .then(|| quote! { #[repr(C)] });
    Ok(quote! {
        #[derive(::ordna_core::__private::facet::Facet)]
        #[facet(crate = ::ordna_core::__private::facet)]
        #repr
        #item
    })
}

// r[impl macro.error.duplicate-path]
/// Records the config path segment of a field, failing if another field already uses it.
fn check_duplicate(
    segments: &mut BTreeMap<String, Span>,
    ident: &Ident,
    segment: Option<&LitStr>,
) -> Result<()> {
    let (name, span) = match segment {
        Some(segment) => (segment.value(), segment.span()),
        None => (unraw(ident), ident.span()),
    };
    if let Some(previous) = segments.insert(name.clone(), span) {
        let mut error = Error::new(span, format!("duplicate config path `{name}`"));
        error.combine(Error::new(previous, "previously used here"));
        return Err(error);
    }
    Ok(())
}

// r[impl macro.error.attributes]
/// Field attributes with `#[config(...)]` replaced by the matching facet attributes, and
/// the overridden path segment, if any.
fn field_attrs(field_attrs: &[Attribute]) -> Result<(Vec<Attribute>, Option<LitStr>)> {
    let mut attrs = Vec::new();
    let mut segment = None::<LitStr>;
    let mut secret = false;
    for attr in field_attrs {
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
        let ty = input.parse()?;
        Ok(Self {
            attrs,
            vis,
            ident,
            ty,
        })
    }
}

impl Parse for FieldType {
    fn parse(input: ParseStream) -> Result<Self> {
        if is_declaration(input, |fork| fork.peek(Token![struct])) {
            return Ok(FieldType::Struct(Box::new(input.parse()?)));
        }
        if is_declaration(input, |fork| fork.peek(Token![enum])) {
            return Ok(FieldType::Enum(Box::new(input.parse()?)));
        }
        // A plain type, unless declarations in its generic arguments make it fail to parse.
        let fork = input.fork();
        if let Ok(ty) = fork.parse::<Type>()
            && (fork.is_empty() || fork.peek(Token![,]) || fork.peek(Token![>]))
        {
            input.advance_to(&fork);
            return Ok(FieldType::Type(ty));
        }

        let mut path = TokenStream2::new();
        if input.peek(Token![::]) {
            input.parse::<Token![::]>()?.to_tokens(&mut path);
        }
        loop {
            input.parse::<Ident>()?.to_tokens(&mut path);
            if !input.peek(Token![::]) {
                break;
            }
            input.parse::<Token![::]>()?.to_tokens(&mut path);
        }
        input.parse::<Token![<]>()?;
        let mut args = vec![input.parse()?];
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.peek(Token![>]) {
                break;
            }
            args.push(input.parse()?);
        }
        input.parse::<Token![>]>()?;
        Ok(FieldType::Generic { path, args })
    }
}

/// Whether the input continues with `[attributes] [visibility]` and then `keyword`.
fn is_declaration(input: ParseStream, keyword: impl FnOnce(ParseStream) -> bool) -> bool {
    let fork = input.fork();
    fork.call(Attribute::parse_outer).is_ok()
        && fork.parse::<Visibility>().is_ok()
        && keyword(&fork)
}

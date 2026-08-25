//! `PassportStrategy` attribute expansion.

use std::hash::{Hash, Hasher};

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Error, Ident, ImplItem, ItemImpl, LitStr, Path, Token, Type, spanned::Spanned};

use crate::path::common_path;

struct StrategyArguments {
    name: LitStr,
}

impl Parse for StrategyArguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if key != "name" {
            return Err(Error::new(
                key.span(),
                "expected `name = \"strategy-name\"`",
            ));
        }
        input.parse::<Token![=]>()?;
        let name: LitStr = input.parse()?;
        if !input.is_empty() {
            return Err(input.error("`#[passport_strategy]` accepts only `name = \"...\"`"));
        }
        validate_name(&name)?;
        Ok(Self { name })
    }
}

/// Expands one concrete managed Passport strategy implementation.
pub(crate) fn expand(arguments: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let arguments = syn::parse2::<StrategyArguments>(arguments)?;
    let implementation = syn::parse2::<ItemImpl>(item)?;
    let common = common_path()?;
    validate_implementation(&implementation)?;
    expand_strategy(arguments, implementation, &common)
}

fn validate_name(name: &LitStr) -> syn::Result<()> {
    let value = name.value();
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(Error::new(
            name.span(),
            "Passport strategy names must be non-empty lowercase ASCII `[a-z0-9._-]`",
        ));
    }
    Ok(())
}

fn validate_implementation(implementation: &ItemImpl) -> syn::Result<()> {
    if !implementation.generics.params.is_empty() || implementation.generics.where_clause.is_some()
    {
        return Err(Error::new(
            implementation.generics.span(),
            "`#[passport_strategy]` requires a non-generic `impl PassportStrategy for ConcreteType`",
        ));
    }
    if implementation.defaultness.is_some() || implementation.unsafety.is_some() {
        return Err(Error::new(
            implementation.impl_token.span,
            "`#[passport_strategy]` requires a safe `impl PassportStrategy for ConcreteType`",
        ));
    }

    let Some((bang, trait_path, _)) = &implementation.trait_ else {
        return Err(Error::new(
            implementation.impl_token.span,
            "`#[passport_strategy]` requires `impl PassportStrategy for ConcreteType`",
        ));
    };
    if bang.is_some() || !is_passport_strategy_path(trait_path) {
        return Err(Error::new(
            trait_path.span(),
            "`#[passport_strategy]` requires `impl PassportStrategy for ConcreteType`",
        ));
    }
    if !matches!(implementation.self_ty.as_ref(), Type::Path(path) if path.qself.is_none()) {
        return Err(Error::new(
            implementation.self_ty.span(),
            "`#[passport_strategy]` requires a concrete strategy type",
        ));
    }

    let mut claims = false;
    let mut principal = false;
    let mut token_kind = false;
    let mut validate = None;
    for item in &implementation.items {
        match item {
            ImplItem::Type(item) if item.ident == "Claims" => claims = true,
            ImplItem::Type(item) if item.ident == "Principal" => principal = true,
            ImplItem::Const(item) if item.ident == "TOKEN_KIND" => token_kind = true,
            ImplItem::Fn(item) if item.sig.ident == "validate" => validate = Some(item),
            _ => {}
        }
    }
    if !claims {
        return Err(Error::new(
            implementation.impl_token.span,
            "`PassportStrategy` implementations require `type Claims = ...;`",
        ));
    }
    if !principal {
        return Err(Error::new(
            implementation.impl_token.span,
            "`PassportStrategy` implementations require `type Principal = ...;`",
        ));
    }
    if !token_kind {
        return Err(Error::new(
            implementation.impl_token.span,
            "`PassportStrategy` implementations require `const TOKEN_KIND: JwtTokenKind = ...;`",
        ));
    }
    let Some(validate) = validate else {
        return Err(Error::new(
            implementation.impl_token.span,
            "`PassportStrategy` implementations require `async fn validate(...)`",
        ));
    };
    if validate.sig.asyncness.is_none() {
        return Err(Error::new(
            validate.sig.fn_token.span,
            "`PassportStrategy::validate` must be async",
        ));
    }
    Ok(())
}

fn is_passport_strategy_path(path: &Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == "PassportStrategy")
}

fn expand_strategy(
    arguments: StrategyArguments,
    implementation: ItemImpl,
    common: &Path,
) -> syn::Result<TokenStream> {
    let strategy_type = implementation.self_ty.as_ref();
    let suffix = generated_suffix(&implementation);
    let adapter = format_ident!("__mads_passport_strategy_adapter_{suffix}");
    let provider_type_id = format_ident!("__mads_passport_strategy_provider_type_id_{suffix}");
    let provider_type_name = format_ident!("__mads_passport_strategy_provider_type_name_{suffix}");
    let claims_type_id = format_ident!("__mads_passport_strategy_claims_type_id_{suffix}");
    let claims_type_name = format_ident!("__mads_passport_strategy_claims_type_name_{suffix}");
    let principal_type_id = format_ident!("__mads_passport_strategy_principal_type_id_{suffix}");
    let principal_type_name =
        format_ident!("__mads_passport_strategy_principal_type_name_{suffix}");
    let name = arguments.name;

    Ok(quote! {
        #implementation

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #adapter<'a>(
            __mads_application: &'a #common::core::ApplicationContext,
            __mads_context: &'a #common::PassportContext<'a>,
            __mads_token: &'a str,
        ) -> #common::PassportStrategyFuture<'a> {
            ::std::boxed::Box::pin(async move {
                let __mads_validation = match <#strategy_type as #common::PassportStrategy>::TOKEN_KIND {
                    #common::JwtTokenKind::Access => #common::JwtValidation::access(),
                    #common::JwtTokenKind::Refresh => #common::JwtValidation::refresh(),
                };
                let __mads_jwt = __mads_application
                    .resolve::<#common::JwtService>()
                    .map_err(|error| #common::PassportError::internal(error))?;
                let __mads_verified = __mads_jwt
                    .verify::<<#strategy_type as #common::PassportStrategy>::Claims>(
                        __mads_token,
                        __mads_validation,
                    )
                    .map_err(#common::PassportError::from)?;
                let __mads_strategy = __mads_application
                    .resolve::<#strategy_type>()
                    .map_err(|error| #common::PassportError::internal(error))?;
                let __mads_principal = <#strategy_type as #common::PassportStrategy>::validate(
                    __mads_strategy.as_ref(),
                    __mads_context,
                    &__mads_verified.claims,
                )
                .await?;
                Ok(#common::ErasedAuthentication::new(
                    __mads_principal,
                    __mads_verified,
                ))
            })
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #provider_type_id() -> ::core::any::TypeId {
            ::core::any::TypeId::of::<#strategy_type>()
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #provider_type_name() -> &'static str {
            ::core::any::type_name::<#strategy_type>()
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #claims_type_id() -> ::core::any::TypeId {
            ::core::any::TypeId::of::<<#strategy_type as #common::PassportStrategy>::Claims>()
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #claims_type_name() -> &'static str {
            ::core::any::type_name::<<#strategy_type as #common::PassportStrategy>::Claims>()
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #principal_type_id() -> ::core::any::TypeId {
            ::core::any::TypeId::of::<<#strategy_type as #common::PassportStrategy>::Principal>()
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #principal_type_name() -> &'static str {
            ::core::any::type_name::<<#strategy_type as #common::PassportStrategy>::Principal>()
        }

        #common::core::__private::inventory::submit! {
            #common::PassportStrategyDescriptor::new(
                #name,
                #provider_type_id,
                #provider_type_name,
                #claims_type_id,
                #claims_type_name,
                #principal_type_id,
                #principal_type_name,
                <#strategy_type as #common::PassportStrategy>::TOKEN_KIND,
                #common::core::SourceLocation::new(file!(), line!(), column!()),
                #adapter,
            )
        }
    })
}

fn generated_suffix(implementation: &ItemImpl) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    implementation
        .to_token_stream()
        .to_string()
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

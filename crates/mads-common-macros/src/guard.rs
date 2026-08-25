//! Parsing and expansion support for inheritable Passport route guards.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Attribute, Error, GenericArgument, Ident, LitStr, Path, Token, Type, bracketed, parenthesized,
    spanned::Spanned,
};

/// The target that owns one guard attribute.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum GuardTarget {
    /// A `#[routes]` trait.
    Trait,
    /// A route-verb method inside that trait.
    Method,
}

/// Guard fields before inheritance is applied.
#[derive(Clone)]
pub(crate) struct GuardSpec {
    strategy: Option<LitStr>,
    principal: Option<Type>,
    source: Option<TokenSourceSpec>,
    roles: Option<PolicyClauseSpec>,
    permissions: Option<PolicyClauseSpec>,
    predicates: Option<Vec<Path>>,
    skip: bool,
    span: Span,
    attribute_span: Span,
    attribute_index: Option<usize>,
}

#[derive(Clone)]
enum TokenSourceSpec {
    Bearer,
    Cookie(LitStr),
}

#[derive(Clone)]
struct PolicyClauseSpec {
    mode: PolicyModeSpec,
    values: Vec<LitStr>,
}

#[derive(Clone, Copy)]
enum PolicyModeSpec {
    Any,
    All,
}

/// One completely resolved route guard ready for static descriptor expansion.
pub(crate) struct EffectiveGuard {
    strategy: LitStr,
    principal: Type,
    source: TokenSourceSpec,
    roles: Option<PolicyClauseSpec>,
    permissions: Option<PolicyClauseSpec>,
    predicates: Vec<Path>,
}

impl Parse for GuardSpec {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let span = input.span();
        let mut result = Self {
            strategy: None,
            principal: None,
            source: None,
            roles: None,
            permissions: None,
            predicates: None,
            skip: false,
            span,
            attribute_span: span,
            attribute_index: None,
        };

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            match key.to_string().as_str() {
                "strategy" => {
                    set_once(&mut result.strategy, key.span(), "strategy")?;
                    input.parse::<Token![=]>()?;
                    let value: LitStr = input.parse()?;
                    validate_strategy_name(&value)?;
                    result.strategy = Some(value);
                }
                "principal" => {
                    set_once(&mut result.principal, key.span(), "principal")?;
                    input.parse::<Token![=]>()?;
                    result.principal = Some(input.parse()?);
                }
                "source" => {
                    set_once(&mut result.source, key.span(), "source")?;
                    input.parse::<Token![=]>()?;
                    result.source = Some(parse_source(input)?);
                }
                "roles" => {
                    set_once(&mut result.roles, key.span(), "roles")?;
                    result.roles = Some(parse_policy_clause(input, "roles")?);
                }
                "permissions" => {
                    set_once(&mut result.permissions, key.span(), "permissions")?;
                    result.permissions = Some(parse_policy_clause(input, "permissions")?);
                }
                "predicate" => {
                    set_once(
                        &mut result.predicates,
                        key.span(),
                        "predicate or predicates",
                    )?;
                    input.parse::<Token![=]>()?;
                    result.predicates = Some(vec![input.parse()?]);
                }
                "predicates" => {
                    set_once(
                        &mut result.predicates,
                        key.span(),
                        "predicate or predicates",
                    )?;
                    input.parse::<Token![=]>()?;
                    let content;
                    bracketed!(content in input);
                    let predicates = Punctuated::<Path, Token![,]>::parse_terminated(&content)?;
                    if predicates.is_empty() {
                        return Err(Error::new(
                            content.span(),
                            "`predicates` must contain at least one path",
                        ));
                    }
                    result.predicates = Some(predicates.into_iter().collect());
                }
                "skip" => {
                    if result.skip {
                        return Err(Error::new(key.span(), "duplicate `skip` guard argument"));
                    }
                    result.skip = true;
                }
                _ => {
                    return Err(Error::new(
                        key.span(),
                        "unknown guard argument; expected strategy, principal, source, roles, permissions, predicate, predicates, or skip",
                    ));
                }
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
        }

        if result.skip && result.has_non_skip_fields() {
            return Err(Error::new(
                result.span,
                "`skip` cannot be combined with other guard arguments",
            ));
        }
        Ok(result)
    }
}

impl GuardSpec {
    fn has_non_skip_fields(&self) -> bool {
        self.strategy.is_some()
            || self.principal.is_some()
            || self.source.is_some()
            || self.roles.is_some()
            || self.permissions.is_some()
            || self.predicates.is_some()
    }

    /// Returns the original position of this attribute on its target.
    pub(crate) fn attribute_index(&self) -> Option<usize> {
        self.attribute_index
    }

    /// Returns the span of the complete guard attribute.
    pub(crate) fn attribute_span(&self) -> Span {
        self.attribute_span
    }
}

fn set_once<T>(slot: &mut Option<T>, span: Span, name: &str) -> syn::Result<()> {
    if slot.is_some() {
        return Err(Error::new(
            span,
            format!("duplicate `{name}` guard argument"),
        ));
    }
    Ok(())
}

fn parse_source(input: ParseStream<'_>) -> syn::Result<TokenSourceSpec> {
    let source: Ident = input.parse()?;
    match source.to_string().as_str() {
        "bearer" => Ok(TokenSourceSpec::Bearer),
        "cookie" => {
            let content;
            parenthesized!(content in input);
            let name: LitStr = content.parse()?;
            if !content.is_empty() {
                return Err(content.error("`cookie` accepts exactly one literal cookie name"));
            }
            validate_cookie_name(&name)?;
            Ok(TokenSourceSpec::Cookie(name))
        }
        _ => Err(Error::new(
            source.span(),
            "guard source must be `bearer` or `cookie(\"literal-name\")`",
        )),
    }
}

fn parse_policy_clause(input: ParseStream<'_>, subject: &str) -> syn::Result<PolicyClauseSpec> {
    let content;
    parenthesized!(content in input);
    let mode: Ident = content.parse()?;
    let mode = match mode.to_string().as_str() {
        "any" => PolicyModeSpec::Any,
        "all" => PolicyModeSpec::All,
        _ => {
            return Err(Error::new(
                mode.span(),
                format!("`{subject}` policy mode must be `any` or `all`"),
            ));
        }
    };
    content.parse::<Token![=]>()?;
    let values;
    bracketed!(values in content);
    let values = Punctuated::<LitStr, Token![,]>::parse_terminated(&values)?;
    if values.is_empty() {
        return Err(Error::new(
            values.span(),
            format!("`{subject}` policy must contain at least one value"),
        ));
    }
    if !content.is_empty() {
        return Err(content.error(format!(
            "`{subject}` accepts exactly one `any = [...]` or `all = [...]` clause"
        )));
    }
    for value in &values {
        validate_policy_value(value, subject)?;
    }
    Ok(PolicyClauseSpec {
        mode,
        values: values.into_iter().collect(),
    })
}

fn validate_strategy_name(value: &LitStr) -> syn::Result<()> {
    if value.value().is_empty()
        || !value.value().bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(Error::new(
            value.span(),
            "guard strategy names must be non-empty lowercase ASCII `[a-z0-9._-]`",
        ));
    }
    Ok(())
}

fn validate_cookie_name(value: &LitStr) -> syn::Result<()> {
    if value.value().is_empty()
        || !value.value().bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
    {
        return Err(Error::new(
            value.span(),
            "guard cookie source must use a non-empty RFC cookie name",
        ));
    }
    Ok(())
}

fn validate_policy_value(value: &LitStr, subject: &str) -> syn::Result<()> {
    if value.value().is_empty() || value.value().chars().any(char::is_control) {
        return Err(Error::new(
            value.span(),
            format!(
                "`{subject}` policy values must be non-empty and contain no control characters"
            ),
        ));
    }
    Ok(())
}

/// Returns whether this is a guard attribute regardless of its facade path.
pub(crate) fn is_guard_attribute(attribute: &Attribute) -> bool {
    attribute
        .path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "guard")
}

/// Removes and parses the one guard attribute permitted on a target.
pub(crate) fn take_guard(
    attributes: &mut Vec<Attribute>,
    target: GuardTarget,
) -> syn::Result<Option<GuardSpec>> {
    let mut found = Vec::new();
    let mut retained = Vec::with_capacity(attributes.len());
    for (index, attribute) in std::mem::take(attributes).into_iter().enumerate() {
        if is_guard_attribute(&attribute) {
            found.push((index, attribute));
        } else {
            retained.push(attribute);
        }
    }
    *attributes = retained;

    if found.len() > 1 {
        return Err(Error::new(
            found[1].1.span(),
            "a route trait or method may declare only one `#[guard(...)]` attribute",
        ));
    }
    let Some((attribute_index, attribute)) = found.pop() else {
        return Ok(None);
    };
    let mut spec = attribute.parse_args::<GuardSpec>()?;
    spec.attribute_span = attribute.span();
    spec.attribute_index = Some(attribute_index);
    if target == GuardTarget::Trait && spec.skip {
        return Err(Error::new(
            attribute.span(),
            "`skip` is only valid on a route method inheriting a trait guard",
        ));
    }
    Ok(Some(spec))
}

/// Combines optional trait and route-method guard fields into one policy.
pub(crate) fn merge(
    trait_guard: Option<&GuardSpec>,
    method_guard: Option<&GuardSpec>,
    method_span: Span,
) -> syn::Result<Option<EffectiveGuard>> {
    let Some(method_guard) = method_guard else {
        return Ok(trait_guard.map(|guard| EffectiveGuard {
            strategy: guard.strategy.clone().expect("validated below"),
            principal: guard.principal.clone().expect("validated below"),
            source: guard.source.clone().unwrap_or(TokenSourceSpec::Bearer),
            roles: guard.roles.clone(),
            permissions: guard.permissions.clone(),
            predicates: guard.predicates.clone().unwrap_or_default(),
        }));
    };

    if method_guard.skip {
        if trait_guard.is_none() {
            return Err(Error::new(
                method_span,
                "`#[guard(skip)]` requires a guard declared on the enclosing `#[routes]` trait",
            ));
        }
        return Ok(None);
    }

    let strategy = method_guard
        .strategy
        .clone()
        .or_else(|| trait_guard.and_then(|guard| guard.strategy.clone()));
    let principal = method_guard
        .principal
        .clone()
        .or_else(|| trait_guard.and_then(|guard| guard.principal.clone()));
    let source = method_guard
        .source
        .clone()
        .or_else(|| trait_guard.and_then(|guard| guard.source.clone()))
        .unwrap_or(TokenSourceSpec::Bearer);
    let roles = method_guard
        .roles
        .clone()
        .or_else(|| trait_guard.and_then(|guard| guard.roles.clone()));
    let permissions = method_guard
        .permissions
        .clone()
        .or_else(|| trait_guard.and_then(|guard| guard.permissions.clone()));
    let predicates = method_guard
        .predicates
        .clone()
        .or_else(|| trait_guard.and_then(|guard| guard.predicates.clone()))
        .unwrap_or_default();

    Ok(Some(EffectiveGuard {
        strategy: strategy.ok_or_else(|| {
            Error::new(
                method_span,
                "a complete guard requires `strategy = \"name\"`",
            )
        })?,
        principal: principal.ok_or_else(|| {
            Error::new(method_span, "a complete guard requires `principal = Type`")
        })?,
        source,
        roles,
        permissions,
        predicates,
    }))
}

/// Validates a trait-only guard before method inheritance occurs.
pub(crate) fn validate_trait_guard(guard: &GuardSpec, span: Span) -> syn::Result<()> {
    if guard.strategy.is_none() {
        return Err(Error::new(
            span,
            "a route-level guard requires `strategy = \"name\"`",
        ));
    }
    if guard.principal.is_none() {
        return Err(Error::new(
            span,
            "a route-level guard requires `principal = Type`",
        ));
    }
    Ok(())
}

impl EffectiveGuard {
    /// Emits one module-level descriptor, its predicate adapters, and its
    /// inventory submission. The route metadata references this exact static.
    pub(crate) fn static_tokens(
        &self,
        common: &syn::Path,
        route_trait: &Ident,
        handler: &Ident,
        conditional_attributes: &[Attribute],
    ) -> (Ident, TokenStream) {
        let static_ident = format_ident!("__mads_guard_{}_{}", route_trait, handler);
        let type_id = format_ident!("__mads_guard_principal_type_id_{}_{}", route_trait, handler);
        let type_name = format_ident!(
            "__mads_guard_principal_type_name_{}_{}",
            route_trait,
            handler
        );
        let principal = &self.principal;
        let trait_name = LitStr::new(&route_trait.to_string(), route_trait.span());
        let handler_name = LitStr::new(&handler.to_string(), handler.span());
        let strategy = &self.strategy;
        let source = source_tokens(&self.source, common);
        let roles = clause_tokens(&self.roles, common);
        let permissions = clause_tokens(&self.permissions, common);
        let predicate_adapters = self
            .predicates
            .iter()
            .enumerate()
            .map(|(index, predicate)| {
                let adapter = format_ident!(
                    "__mads_guard_predicate_{}_{}_{}",
                    route_trait,
                    handler,
                    index
                );
                quote! {
                    #(#conditional_attributes)*
                    #[doc(hidden)]
                    #[allow(non_snake_case)]
                    fn #adapter(
                        __mads_authentication: &#common::ErasedAuthentication,
                    ) -> bool {
                        let __mads_predicate: fn(&#principal) -> bool = #predicate;
                        let Some(__mads_principal) =
                            __mads_authentication.principal_as::<#principal>()
                        else {
                            return false;
                        };
                        __mads_predicate(__mads_principal.as_ref())
                    }
                }
            });
        let predicate_entries = self
            .predicates
            .iter()
            .enumerate()
            .map(|(index, predicate)| {
                let adapter = format_ident!(
                    "__mads_guard_predicate_{}_{}_{}",
                    route_trait,
                    handler,
                    index
                );
                quote!(#common::GuardPredicate::new(stringify!(#predicate), Some(#adapter)))
            });
        let (builtin_function, builtin) = builtin_adapter_tokens(
            &self.principal,
            common,
            route_trait,
            handler,
            conditional_attributes,
        );

        let tokens = quote! {
            #(#conditional_attributes)*
            #[doc(hidden)]
            #[allow(non_snake_case)]
            fn #type_id() -> ::core::any::TypeId {
                ::core::any::TypeId::of::<#principal>()
            }

            #(#conditional_attributes)*
            #[doc(hidden)]
            #[allow(non_snake_case)]
            fn #type_name() -> &'static str {
                ::core::any::type_name::<#principal>()
            }

            #(#predicate_adapters)*
            #builtin_function

            #(#conditional_attributes)*
            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            static #static_ident: #common::GuardDescriptor = #common::GuardDescriptor::new(
                #trait_name,
                #handler_name,
                #strategy,
                Some(#type_id),
                Some(#type_name),
                #source,
                #roles,
                #permissions,
                &[#(#predicate_entries,)*],
                #common::core::SourceLocation::new(file!(), line!(), column!()),
                #builtin,
            )
            .with_requirement_subject(concat!(stringify!(#route_trait), "::", stringify!(#handler)));

            #(#conditional_attributes)*
            #common::core::__private::inventory::submit! {
                &#static_ident
            }
        };
        (static_ident, tokens)
    }
}

fn source_tokens(source: &TokenSourceSpec, common: &syn::Path) -> TokenStream {
    match source {
        TokenSourceSpec::Bearer => quote!(#common::TokenSource::Bearer),
        TokenSourceSpec::Cookie(name) => quote!(#common::TokenSource::Cookie(#name)),
    }
}

fn clause_tokens(clause: &Option<PolicyClauseSpec>, common: &syn::Path) -> TokenStream {
    let Some(clause) = clause else {
        return quote!(None);
    };
    let mode = match clause.mode {
        PolicyModeSpec::Any => quote!(#common::PolicyMode::Any),
        PolicyModeSpec::All => quote!(#common::PolicyMode::All),
    };
    let values = &clause.values;
    quote!(Some(#common::PolicyClause::new(#mode, &[#(#values,)*])))
}

fn builtin_adapter_tokens(
    principal: &Type,
    common: &syn::Path,
    route_trait: &Ident,
    handler: &Ident,
    conditional_attributes: &[Attribute],
) -> (TokenStream, TokenStream) {
    let Some(claims) = claims_principal_claims(principal) else {
        return (TokenStream::new(), quote!(None));
    };
    let adapter = format_ident!("__mads_guard_builtin_jwt_{}_{}", route_trait, handler);
    (
        quote! {
            #(#conditional_attributes)*
            #[doc(hidden)]
            #[allow(non_snake_case)]
            fn #adapter<'a>(
                __mads_application: &'a #common::core::ApplicationContext,
                _context: &'a #common::PassportContext<'a>,
                __mads_token: &'a str,
            ) -> #common::PassportStrategyFuture<'a> {
                ::std::boxed::Box::pin(async move {
                    let __mads_jwt = __mads_application
                        .resolve::<#common::JwtService>()
                        .map_err(#common::PassportError::internal)?;
                    let __mads_verified = ::std::sync::Arc::new(
                        __mads_jwt
                            .verify::<#claims>(
                                __mads_token,
                                #common::JwtValidation::access(),
                            )
                            .map_err(#common::PassportError::from)?,
                    );
                    let __mads_principal = #common::ClaimsPrincipal::<#claims>::new(
                        ::std::sync::Arc::clone(&__mads_verified),
                    );
                    Ok(#common::ErasedAuthentication::with_verified(
                        __mads_principal,
                        __mads_verified,
                    ))
                })
            }
        },
        quote!(Some(#adapter)),
    )
}

fn claims_principal_claims(principal: &Type) -> Option<Type> {
    let Type::Path(path) = principal else {
        return None;
    };
    if path.qself.is_some() || path.path.segments.last()?.ident != "ClaimsPrincipal" {
        return None;
    }
    let segment = path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    if arguments.args.len() != 1 {
        return None;
    }
    match arguments.args.first()? {
        GenericArgument::Type(claims) => Some(claims.clone()),
        _ => None,
    }
}

/// Emits a focused error for `#[guard]` that was not consumed by `#[routes]`.
pub(crate) fn outside_contract(arguments: TokenStream, item: TokenStream) -> TokenStream {
    let span = if arguments.is_empty() {
        Span::call_site()
    } else {
        arguments.span()
    };
    let error = Error::new(
        span,
        "`#[guard]` must appear below `#[routes]` on that trait or below one route verb on a method",
    )
    .into_compile_error();
    quote!(#item #error)
}

#[cfg(test)]
#[path = "../tests/support/guard.rs"]
mod tests;

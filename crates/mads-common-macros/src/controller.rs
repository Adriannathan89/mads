//! Expansion for managed controllers associated with route traits.

use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::visit_mut::{self, VisitMut};
use syn::{
    Attribute, Error, ExprPath, Fields, Ident, ItemStruct, Path, Token, Type, TypePath, bracketed,
    spanned::Spanned,
};

use crate::path::common_path;

struct ControllerArguments {
    routes: Vec<Path>,
}

impl Parse for ControllerArguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        if name != "routes" {
            return Err(Error::new(name.span(), "expected `routes = [RouteTrait]`"));
        }
        input.parse::<Token![=]>()?;
        let content;
        bracketed!(content in input);
        let routes = Punctuated::<Path, Token![,]>::parse_terminated(&content)?;
        if !input.is_empty() {
            return Err(input.error("`#[controller]` accepts only one `routes` argument"));
        }
        if routes.is_empty() {
            return Err(Error::new(
                content.span(),
                "`#[controller]` requires at least one route trait",
            ));
        }

        let mut unique = BTreeSet::new();
        for route in &routes {
            let identity = route.to_token_stream().to_string();
            if !unique.insert(identity) {
                return Err(Error::new(route.span(), "duplicate controller route trait"));
            }
        }

        Ok(Self {
            routes: routes.into_iter().collect(),
        })
    }
}

/// Expands a controller into a managed handle and route-contract assertions.
pub(crate) fn expand(arguments: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let arguments = syn::parse2::<ControllerArguments>(arguments)?;
    let item: ItemStruct = syn::parse2(item)?;
    expand_controller(arguments, item)
}

fn expand_controller(arguments: ControllerArguments, item: ItemStruct) -> syn::Result<TokenStream> {
    if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
        return Err(Error::new(
            item.generics.span(),
            "`#[controller]` supports only non-generic named-field or unit structs",
        ));
    }
    if let Fields::Unnamed(fields) = &item.fields {
        return Err(Error::new(
            fields.span(),
            "`#[controller]` supports only named-field or unit structs",
        ));
    }
    if let Some(attribute) = item.attrs.iter().find(is_repr) {
        return Err(Error::new(
            attribute.span(),
            "representation attributes are not supported on `#[controller]` structs",
        ));
    }
    if let Some(attribute) = item
        .attrs
        .iter()
        .find(|attribute| !is_supported_attribute(attribute))
    {
        return Err(Error::new(
            attribute.span(),
            "`#[controller]` structs support documentation and lint attributes only in v0.1",
        ));
    }
    if let Fields::Named(fields) = &item.fields {
        for field in &fields.named {
            if let Some(attribute) = field
                .attrs
                .iter()
                .find(|attribute| !is_supported_attribute(attribute))
            {
                return Err(Error::new(
                    attribute.span(),
                    "`#[controller]` fields support documentation and lint attributes only in v0.1",
                ));
            }
        }
    }

    let common = common_path()?;
    let core = quote!(#common::core);
    let generated_suffix = generated_suffix(&item, &item.ident);
    let ItemStruct {
        attrs,
        vis,
        ident,
        fields,
        ..
    } = item;
    let inner_ident = format_ident!("__mads_controller_inner_{generated_suffix}");
    let constructor_ident = format_ident!("__mads_construct_controller_{generated_suffix}");
    let is_unit = matches!(fields, Fields::Unit);

    let (inner_fields, resolve_fields, dependencies) = match fields {
        Fields::Named(fields) => {
            let normalized_fields: Vec<_> = fields
                .named
                .iter()
                .cloned()
                .map(|mut field| {
                    normalize_self_type(&mut field.ty, &ident);
                    field
                })
                .collect();
            let declarations = normalized_fields.iter().map(|field| {
                let attrs = &field.attrs;
                let vis = &field.vis;
                let ident = &field.ident;
                let ty = &field.ty;
                quote!(#(#attrs)* #vis #ident: #ty)
            });
            let resolutions = normalized_fields.iter().map(|field| {
                let ident = field.ident.as_ref().expect("named fields have identifiers");
                let ty = &field.ty;
                quote!(#ident: context.resolve::<#ty>()?.as_ref().clone())
            });
            let descriptors = normalized_fields.iter().map(|field| {
                let ty = &field.ty;
                quote! {
                    #core::DependencyDescriptor::new(
                        stringify!(#ty),
                        || ::core::any::TypeId::of::<#ty>(),
                    )
                }
            });
            (
                quote!({ #(#declarations,)* }),
                quote!({ #(#resolutions,)* }),
                quote!(&[#(#descriptors,)*]),
            )
        }
        Fields::Unit => (quote!(;), quote!(), quote!(&[])),
        Fields::Unnamed(_) => unreachable!("tuple fields were rejected above"),
    };

    let inner_value = if is_unit {
        quote!(#inner_ident)
    } else {
        quote!(#inner_ident #resolve_fields)
    };
    let route_assertions = arguments
        .routes
        .iter()
        .map(|route| quote!(let _ = <#ident as #route>::__MADS_ROUTE_CONTRACT;));
    let route_contracts = arguments.routes.iter().map(|route| {
        quote! {
            #common::RouteContractDescriptor::new(
                stringify!(#route),
                <#ident as #route>::__MADS_ROUTE_METADATA,
            )
        }
    });

    Ok(quote! {
        #[doc(hidden)]
        #vis struct #inner_ident #inner_fields

        #(#attrs)*
        #vis struct #ident(::std::sync::Arc<#inner_ident>);

        impl ::core::clone::Clone for #ident {
            fn clone(&self) -> Self {
                Self(::std::sync::Arc::clone(&self.0))
            }
        }

        impl ::core::ops::Deref for #ident {
            type Target = #inner_ident;

            fn deref(&self) -> &Self::Target {
                self.0.as_ref()
            }
        }

        const _: () = {
            #(#route_assertions)*
        };

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #constructor_ident<'a>(
            context: &'a #core::ConstructionContext<'a>,
        ) -> #core::ProviderFuture<'a> {
            ::std::boxed::Box::pin(async move {
                #common::RouteCatalog::validate_controller::<#ident>()?;
                let value = #ident(::std::sync::Arc::new(#inner_value));
                let erased: #core::ErasedProvider = ::std::sync::Arc::new(value);
                Ok(erased)
            })
        }

        #core::__private::inventory::submit! {
            #core::ProviderDescriptor::new(
                #core::ProviderKind::Service,
                concat!(module_path!(), "::", stringify!(#ident)),
                || ::core::any::TypeId::of::<#ident>(),
                #dependencies,
                #core::SourceLocation::new(file!(), line!(), column!()),
                #constructor_ident,
            )
        }

        #core::__private::inventory::submit! {
            #common::ControllerRouteDescriptor::new(
                concat!(module_path!(), "::", stringify!(#ident)),
                || ::core::any::TypeId::of::<#ident>(),
                &[#(#route_contracts,)*],
            )
        }
    })
}

fn generated_suffix(item: &ItemStruct, ident: &Ident) -> String {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    item.to_token_stream().to_string().hash(&mut hasher);
    ident.hash(&mut hasher);
    format!("{ident}_{:016x}", hasher.finish())
}

fn is_repr(attribute: &&Attribute) -> bool {
    attribute.path().is_ident("repr")
}

fn is_doc(attribute: &&Attribute) -> bool {
    attribute.path().is_ident("doc")
}

fn is_supported_attribute(attribute: &&Attribute) -> bool {
    is_doc(attribute)
        || attribute.path().is_ident("allow")
        || attribute.path().is_ident("warn")
        || attribute.path().is_ident("deny")
        || attribute.path().is_ident("forbid")
}

fn normalize_self_type(ty: &mut Type, handle: &Ident) {
    SelfTypeNormalizer { handle }.visit_type_mut(ty);
}

struct SelfTypeNormalizer<'a> {
    handle: &'a Ident,
}

impl VisitMut for SelfTypeNormalizer<'_> {
    fn visit_expr_path_mut(&mut self, expression_path: &mut ExprPath) {
        if expression_path.qself.is_none() {
            if let Some(segment) = expression_path.path.segments.first_mut() {
                if segment.ident == "Self" {
                    segment.ident = self.handle.clone();
                }
            }
        }
        visit_mut::visit_expr_path_mut(self, expression_path);
    }

    fn visit_type_path_mut(&mut self, type_path: &mut TypePath) {
        if type_path.qself.is_none() {
            if let Some(segment) = type_path.path.segments.first_mut() {
                if segment.ident == "Self" {
                    segment.ident = self.handle.clone();
                }
            }
        }
        visit_mut::visit_type_path_mut(self, type_path);
    }
}
#[cfg(test)]
include!("../tests/support/controller.rs");

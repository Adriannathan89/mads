//! Expansion for statically registered application modules.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Error, Fields, ItemStruct, spanned::Spanned};

use crate::path::core_path;

const SUPPORTED_FORM: &str = "`#[mads::module]` supports only `#[mads::module] struct AppModule;`";

/// Expands a supported module declaration into the declaration and its metadata.
pub(crate) fn expand(arguments: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !arguments.is_empty() {
        return Err(Error::new(arguments.span(), SUPPORTED_FORM));
    }

    let item: ItemStruct = syn::parse2(item)?;
    if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
        return Err(Error::new(item.generics.span(), SUPPORTED_FORM));
    }
    if !matches!(item.fields, Fields::Unit) {
        return Err(Error::new(item.fields.span(), SUPPORTED_FORM));
    }

    let core = core_path()?;
    let ident = &item.ident;

    Ok(quote! {
        #item

        #core::__private::inventory::submit! {
            #core::ModuleDescriptor::new(
                concat!(module_path!(), "::", stringify!(#ident)),
                || ::core::any::TypeId::of::<#ident>(),
                #core::SourceLocation::new(file!(), line!(), column!()),
            )
        }
    })
}
#[cfg(test)]
include!("../tests/support/module.rs");

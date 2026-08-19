//! Expansion for a Tokio-backed asynchronous application entry point.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Error, ItemFn, spanned::Spanned};

use crate::path::core_path;

/// Expands an asynchronous main function into a synchronous runtime bootstrap.
pub(crate) fn expand(arguments: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !arguments.is_empty() {
        return Err(Error::new(
            arguments.span(),
            "`#[mads::main]` does not accept arguments",
        ));
    }

    let item: ItemFn = syn::parse2(item)?;
    validate_signature(&item)?;
    let core = core_path()?;
    let attrs = &item.attrs;
    let visibility = &item.vis;
    let output = &item.sig.output;
    let body = &item.block;

    Ok(quote! {
        #(#attrs)*
        #visibility fn main() #output {
            #core::runtime::block_on(async move #body)
        }
    })
}

fn validate_signature(item: &ItemFn) -> syn::Result<()> {
    if item.sig.ident != "main" {
        return Err(Error::new(
            item.sig.ident.span(),
            "`#[mads::main]` can only be applied to a function named `main`",
        ));
    }
    if item.sig.asyncness.is_none() {
        return Err(Error::new(
            item.sig.fn_token.span(),
            "`#[mads::main]` requires an asynchronous function",
        ));
    }
    if !item.sig.inputs.is_empty() {
        return Err(Error::new(
            item.sig.inputs.span(),
            "`#[mads::main]` does not support function arguments",
        ));
    }
    if !item.sig.generics.params.is_empty() || item.sig.generics.where_clause.is_some() {
        return Err(Error::new(
            item.sig.generics.span(),
            "`#[mads::main]` does not support generic parameters or where clauses",
        ));
    }
    if let Some(constness) = &item.sig.constness {
        return Err(Error::new(
            constness.span(),
            "`#[mads::main]` does not support const functions",
        ));
    }
    if let Some(unsafety) = &item.sig.unsafety {
        return Err(Error::new(
            unsafety.span(),
            "`#[mads::main]` does not support unsafe functions",
        ));
    }
    if let Some(abi) = &item.sig.abi {
        return Err(Error::new(
            abi.span(),
            "`#[mads::main]` does not support extern functions",
        ));
    }
    Ok(())
}
#[cfg(test)]
include!("../tests/support/main_fixture.rs");

//! Procedural macros for declaring MADS.rs modules and managed providers.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use proc_macro::TokenStream;

mod managed;
mod module;
mod path;

/// Declares a non-generic unit struct as an application module.
#[proc_macro_attribute]
pub fn module(arguments: TokenStream, item: TokenStream) -> TokenStream {
    module::expand(arguments.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declares a named-field or unit struct as an application service.
#[proc_macro_attribute]
pub fn service(arguments: TokenStream, item: TokenStream) -> TokenStream {
    managed::expand(managed::ManagedKind::Service, arguments.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declares a named-field or unit struct as a persistence repository.
#[proc_macro_attribute]
pub fn repository(arguments: TokenStream, item: TokenStream) -> TokenStream {
    managed::expand(
        managed::ManagedKind::Repository,
        arguments.into(),
        item.into(),
    )
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}

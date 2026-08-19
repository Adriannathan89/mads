//! Procedural macros for compile-time controller and route contracts.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use proc_macro::TokenStream;

mod controller;
mod path;
mod routes;
mod verb;

/// Declares a managed controller and the route traits it must implement.
#[proc_macro_attribute]
pub fn controller(arguments: TokenStream, item: TokenStream) -> TokenStream {
    controller::expand(arguments.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declares and validates a trait containing HTTP route contracts.
#[proc_macro_attribute]
pub fn routes(arguments: TokenStream, item: TokenStream) -> TokenStream {
    routes::expand(arguments.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Marks a GET method inside a trait annotated with [`routes`].
#[proc_macro_attribute]
pub fn get(arguments: TokenStream, item: TokenStream) -> TokenStream {
    verb::outside_contract("get", arguments.into(), item.into()).into()
}

/// Marks a POST method inside a trait annotated with [`routes`].
#[proc_macro_attribute]
pub fn post(arguments: TokenStream, item: TokenStream) -> TokenStream {
    verb::outside_contract("post", arguments.into(), item.into()).into()
}

/// Marks a PUT method inside a trait annotated with [`routes`].
#[proc_macro_attribute]
pub fn put(arguments: TokenStream, item: TokenStream) -> TokenStream {
    verb::outside_contract("put", arguments.into(), item.into()).into()
}

/// Marks a PATCH method inside a trait annotated with [`routes`].
#[proc_macro_attribute]
pub fn patch(arguments: TokenStream, item: TokenStream) -> TokenStream {
    verb::outside_contract("patch", arguments.into(), item.into()).into()
}

/// Marks a DELETE method inside a trait annotated with [`routes`].
#[proc_macro_attribute]
pub fn delete(arguments: TokenStream, item: TokenStream) -> TokenStream {
    verb::outside_contract("delete", arguments.into(), item.into()).into()
}

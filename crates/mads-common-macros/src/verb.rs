//! Guard expansion for endpoint attributes used outside route contracts.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Error, spanned::Spanned};

/// Emits a focused diagnostic when an endpoint attribute survives `#[routes]`.
pub(crate) fn outside_contract(
    verb: &str,
    arguments: TokenStream,
    item: TokenStream,
) -> TokenStream {
    let span = if arguments.is_empty() {
        Span::call_site()
    } else {
        arguments.span()
    };
    let error = Error::new(
        span,
        format!("`#[{verb}]` must be used on a method inside a trait annotated with `#[routes]`"),
    )
    .into_compile_error();

    quote!(#item #error)
}

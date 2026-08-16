//! Expansion paths that work through the core crate or public facade.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use syn::{Error, Path, parse_quote};

/// Resolves the core runtime path visible to the macro consumer.
pub(crate) fn core_path() -> syn::Result<Path> {
    if let Ok(found) = crate_name("mads-core") {
        return found_path(found, false);
    }

    if let Ok(found) = crate_name("mads") {
        return found_path(found, true);
    }

    Err(Error::new(
        Span::call_site(),
        "MADS attributes require a dependency on `mads-core` or `mads`",
    ))
}

fn found_path(found: FoundCrate, facade: bool) -> syn::Result<Path> {
    match (found, facade) {
        (FoundCrate::Itself, false) => Ok(parse_quote!(crate)),
        (FoundCrate::Itself, true) => Ok(parse_quote!(::mads::core)),
        (FoundCrate::Name(name), false) => named_path(&name, false),
        (FoundCrate::Name(name), true) => named_path(&name, true),
    }
}

fn named_path(name: &str, facade: bool) -> syn::Result<Path> {
    let ident = syn::Ident::new(&name.replace('-', "_"), Span::call_site());
    if facade {
        Ok(parse_quote!(::#ident::core))
    } else {
        Ok(parse_quote!(::#ident))
    }
}

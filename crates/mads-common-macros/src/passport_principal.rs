//! `PassportPrincipal` derive expansion.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, Ident, Result, spanned::Spanned};

use crate::path::common_path;

pub(crate) fn expand(input: DeriveInput) -> Result<TokenStream> {
    reject_item_markers(&input)?;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            Fields::Unnamed(fields) => {
                return Err(Error::new(
                    fields.span(),
                    "`PassportPrincipal` supports only named-field structs",
                ));
            }
            Fields::Unit => {
                return Err(Error::new(
                    input.ident.span(),
                    "`PassportPrincipal` supports only named-field structs",
                ));
            }
        },
        Data::Enum(data) => {
            return Err(Error::new(
                data.enum_token.span(),
                "`PassportPrincipal` supports only named-field structs",
            ));
        }
        Data::Union(data) => {
            return Err(Error::new(
                data.union_token.span(),
                "`PassportPrincipal` supports only named-field structs",
            ));
        }
    };

    let mut roles = None;
    let mut permissions = None;
    for field in fields {
        let field_roles = marker_count(field.attrs.iter(), "roles");
        let field_permissions = marker_count(field.attrs.iter(), "permissions");
        if field_roles > 1 || field_permissions > 1 {
            return Err(Error::new(
                field.span(),
                "Passport principal policy marker may appear only once per field",
            ));
        }
        if field_roles == 1 && field_permissions == 1 {
            return Err(Error::new(
                field.span(),
                "a Passport principal field cannot be both `roles` and `permissions`",
            ));
        }

        let ident = field.ident.as_ref().expect("named fields have identifiers");
        if field_roles == 1 {
            set_unique(&mut roles, ident, "roles")?;
        }
        if field_permissions == 1 {
            set_unique(&mut permissions, ident, "permissions")?;
        }
    }

    let common = common_path()?;
    let ident = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let role_body = membership_body(roles);
    let permission_body = membership_body(permissions);

    Ok(quote! {
        impl #impl_generics #common::PassportPrincipal for #ident #type_generics #where_clause {
            fn has_role(&self, requested: &str) -> bool { #role_body }
            fn has_permission(&self, requested: &str) -> bool { #permission_body }
        }
    })
}

fn reject_item_markers(input: &DeriveInput) -> Result<()> {
    if let Some(attribute) = input.attrs.iter().find(|attribute| {
        attribute.path().is_ident("roles") || attribute.path().is_ident("permissions")
    }) {
        return Err(Error::new(
            attribute.span(),
            "Passport principal policy markers are valid only on named fields",
        ));
    }
    Ok(())
}

fn marker_count<'a>(attributes: impl Iterator<Item = &'a syn::Attribute>, marker: &str) -> usize {
    attributes
        .filter(|attribute| attribute.path().is_ident(marker))
        .count()
}

fn set_unique(slot: &mut Option<Ident>, ident: &Ident, marker: &str) -> Result<()> {
    if slot.is_some() {
        return Err(Error::new(
            ident.span(),
            format!("duplicate `#[{marker}]` Passport principal field"),
        ));
    }
    *slot = Some(ident.clone());
    Ok(())
}

fn membership_body(field: Option<Ident>) -> TokenStream {
    field.map_or_else(
        || quote!(false),
        |field| {
            quote! {
                self.#field.iter().any(|value| {
                    ::core::convert::AsRef::<str>::as_ref(value) == requested
                })
            }
        },
    )
}

//! Expansion for synchronous and asynchronous provider functions.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::visit::{self, Visit};
use syn::{
    Error, FnArg, GenericArgument, GenericParam, ItemFn, PathArguments, ReturnType, Type,
    spanned::Spanned,
};

use crate::path::core_path;

/// Expands a provider function into the original function and registered constructor metadata.
pub(crate) fn expand(arguments: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !arguments.is_empty() {
        return Err(Error::new(
            arguments.span(),
            "`#[mads::provider]` does not accept arguments",
        ));
    }

    let item: ItemFn = syn::parse2(item)?;
    validate_signature(&item)?;
    expand_provider(item)
}

fn validate_signature(item: &ItemFn) -> syn::Result<()> {
    if let Some(receiver) = item.sig.inputs.iter().find_map(|argument| match argument {
        FnArg::Receiver(receiver) => Some(receiver),
        FnArg::Typed(_) => None,
    }) {
        return Err(Error::new(
            receiver.span(),
            "`#[mads::provider]` cannot be applied to methods; declare a free function without a `self` receiver",
        ));
    }

    if let Some(parameter) = item
        .sig
        .generics
        .params
        .iter()
        .find(|parameter| matches!(parameter, GenericParam::Type(_) | GenericParam::Const(_)))
    {
        return Err(Error::new(
            parameter.span(),
            "`#[mads::provider]` does not support type or const generics",
        ));
    }

    if let Some(variadic) = &item.sig.variadic {
        return Err(Error::new(
            variadic.span(),
            "`#[mads::provider]` does not support variadic functions",
        ));
    }

    if let Some(unsafety) = &item.sig.unsafety {
        return Err(Error::new(
            unsafety.span(),
            "`#[mads::provider]` does not support unsafe functions",
        ));
    }

    match &item.sig.output {
        ReturnType::Default => Err(Error::new(
            item.sig.ident.span(),
            "`#[mads::provider]` requires an explicit concrete return type",
        )),
        ReturnType::Type(_, output) => non_concrete_type(output).map_or(Ok(()), |non_concrete| {
            Err(Error::new(
                non_concrete.span(),
                "`#[mads::provider]` requires an explicit concrete return type",
            ))
        }),
    }
}

fn expand_provider(item: ItemFn) -> syn::Result<TokenStream> {
    let core = core_path()?;
    let ident = &item.sig.ident;
    let return_type = match &item.sig.output {
        ReturnType::Type(_, return_type) => return_type.as_ref(),
        ReturnType::Default => unreachable!("provider return type was validated"),
    };
    let (output_type, fallible) =
        result_output(return_type).map_or((return_type, false), |output_type| (output_type, true));

    let dependencies: Vec<_> = item
        .sig
        .inputs
        .iter()
        .filter_map(|argument| match argument {
            FnArg::Typed(argument) => Some(argument.ty.as_ref()),
            FnArg::Receiver(_) => None,
        })
        .collect();
    let dependency_idents: Vec<_> = (0..dependencies.len())
        .map(|index| format_ident!("__mads_dependency_{index}"))
        .collect();
    let resolve_dependencies =
        dependencies
            .iter()
            .zip(&dependency_idents)
            .map(|(dependency, dependency_ident)| {
                quote! {
                let #dependency_ident = context
                    .resolve::<#dependency>()?;
                let #dependency_ident =
                    ::core::clone::Clone::clone(#dependency_ident.as_ref());
                }
            });
    let dependency_descriptors = dependencies.iter().map(|dependency| {
        quote! {
            #core::DependencyDescriptor::new(
                stringify!(#dependency),
                || ::core::any::TypeId::of::<#dependency>(),
            )
        }
    });
    let call = quote!(#ident(#(#dependency_idents),*));
    let call = if item.sig.asyncness.is_some() {
        quote!(#call.await)
    } else {
        call
    };
    let construct_value = if fallible {
        quote!(let value: #output_type = #call?;)
    } else {
        quote!(let value: #output_type = #call;)
    };

    Ok(quote! {
        #item

        const _: () = {
            #[doc(hidden)]
            fn __mads_construct<'a>(
                context: &'a #core::ConstructionContext<'a>,
            ) -> #core::ProviderFuture<'a> {
                ::std::boxed::Box::pin(async move {
                    #(#resolve_dependencies)*
                    #construct_value
                    let erased: #core::ErasedProvider = ::std::sync::Arc::new(value);
                    Ok(erased)
                })
            }

            #core::__private::inventory::submit! {
                #core::ProviderDescriptor::new(
                    #core::ProviderKind::Provider,
                    stringify!(#output_type),
                    || ::core::any::TypeId::of::<#output_type>(),
                    &[#(#dependency_descriptors,)*],
                    #core::SourceLocation::new(file!(), line!(), column!()),
                    __mads_construct,
                )
            }
        };
    })
}

fn non_concrete_type(output: &Type) -> Option<&Type> {
    let mut finder = NonConcreteTypeFinder { found: None };
    finder.visit_type(output);
    finder.found
}

struct NonConcreteTypeFinder<'ast> {
    found: Option<&'ast Type>,
}

impl<'ast> Visit<'ast> for NonConcreteTypeFinder<'ast> {
    fn visit_type(&mut self, node: &'ast Type) {
        if self.found.is_some() {
            return;
        }
        if matches!(node, Type::Infer(_) | Type::ImplTrait(_)) {
            self.found = Some(node);
            return;
        }
        visit::visit_type(self, node);
    }
}

fn result_output(return_type: &Type) -> Option<&Type> {
    let Type::Path(type_path) = return_type else {
        return None;
    };
    if type_path.qself.is_some() {
        return None;
    }

    let names: Vec<_> = type_path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    let recognized = match names.as_slice() {
        [result] => result == "Result",
        [core, result] => core == "mads_core" && result == "Result",
        [facade, core, result] => facade == "mads" && core == "core" && result == "Result",
        _ => false,
    };
    if !recognized {
        return None;
    }

    let PathArguments::AngleBracketed(arguments) = &type_path.path.segments.last()?.arguments
    else {
        return None;
    };
    if arguments.args.len() != 1 {
        return None;
    }
    match arguments.args.first()? {
        GenericArgument::Type(output) => Some(output),
        _ => None,
    }
}

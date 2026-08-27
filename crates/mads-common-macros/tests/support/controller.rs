//! Unit tests for managed-controller expansion.

use super::*;
use quote::ToTokens;

fn normalized(tokens: impl ToTokens) -> String {
    tokens
        .to_token_stream()
        .to_string()
        .split_whitespace()
        .collect()
}

#[test]
fn expands_a_concrete_controller_registrar_and_stores_its_pointer() {
    let arguments: ControllerArguments = syn::parse_str("routes = [UserRoutes, AdminRoutes]")
        .expect("controller arguments should parse");
    let item: ItemStruct =
        syn::parse_str("pub struct Controller;").expect("controller should parse");
    let expanded = expand_controller_with_common(arguments, item, &syn::parse_quote!(mads_common))
        .expect("controller should expand");
    let expanded = normalized(expanded);

    assert_eq!(
        expanded
            .match_indices(&normalized(quote!(__mads_context.resolve::<Controller>()?)))
            .count(),
        1,
        "the registrar must resolve Controller exactly once",
    );
    assert!(expanded.contains(&normalized(quote! {
        <Controller as UserRoutes>::__mads_register(
            __mads_router,
            __mads_controller.clone(),
            __mads_context,
            __mads_routes,
        )?
    })));
    assert!(expanded.contains(&normalized(quote! {
        <Controller as AdminRoutes>::__mads_register(
            __mads_router,
            __mads_controller.clone(),
            __mads_context,
            __mads_routes,
        )?
    })));
    assert!(expanded.contains(&normalized(quote!(__mads_routes.finish()?))));
    assert!(expanded.contains("ControllerRouteDescriptor::with_registrar"));
    assert!(expanded.contains(&normalized(quote! {
        .with_runtime_type_name(|| ::core::any::type_name::<Controller>())
    })));
    assert_eq!(
        expanded.matches("with_namespace(module_path!())").count(),
        2,
        "the provider and controller descriptors must retain their declaration namespace",
    );
    assert!(expanded.contains("__mads_register_controller_"));
}

#[test]
fn parses_controller_route_arguments_and_rejects_duplicates() {
    let arguments: ControllerArguments = syn::parse_str("routes = [UserRoute, admin::AdminRoute]")
        .expect("controller arguments should parse");
    assert_eq!(arguments.routes.len(), 2);
    assert!(syn::parse_str::<ControllerArguments>("").is_err());
    assert!(syn::parse_str::<ControllerArguments>("route = [UserRoute]").is_err());
    assert!(syn::parse_str::<ControllerArguments>("routes = []").is_err());
    assert!(syn::parse_str::<ControllerArguments>("routes = [UserRoute, UserRoute]").is_err());
    assert!(syn::parse_str::<ControllerArguments>("routes = [UserRoute] extra").is_err());
}

#[test]
fn generated_suffix_is_stable_and_sensitive_to_the_struct_shape() {
    let first: ItemStruct = syn::parse_str("struct Controller;").unwrap();
    let second: ItemStruct = syn::parse_str("struct Controller { value: i32 }").unwrap();
    let ident: Ident = syn::parse_str("Controller").unwrap();
    assert_eq!(
        generated_suffix(&first, &ident),
        generated_suffix(&first, &ident)
    );
    assert_ne!(
        generated_suffix(&first, &ident),
        generated_suffix(&second, &ident)
    );
}

#[test]
fn normalizes_self_types_in_nested_fields_and_expressions() {
    let handle: Ident = syn::parse_str("Controller").unwrap();
    let mut ty: Type = syn::parse_str("Option<Self>").unwrap();
    normalize_self_type(&mut ty, &handle);
    assert_eq!(ty.to_token_stream().to_string(), "Option < Controller >");

    let mut expression: ExprPath = syn::parse_str("Self::new").unwrap();
    SelfTypeNormalizer { handle: &handle }.visit_expr_path_mut(&mut expression);
    assert_eq!(
        expression.to_token_stream().to_string(),
        "Controller :: new"
    );
}

#[test]
fn classifies_controller_attributes() {
    let item: ItemStruct = syn::parse_str(
        "#[repr(C)] #[doc = \"docs\"] #[allow(dead_code)] #[derive(Clone)] struct Controller;",
    )
    .unwrap();
    assert!(is_repr(&&item.attrs[0]));
    assert!(is_doc(&&item.attrs[1]));
    assert!(is_supported_attribute(&&item.attrs[1]));
    assert!(is_supported_attribute(&&item.attrs[2]));
    assert!(!is_supported_attribute(&&item.attrs[3]));
}

#[test]
fn rejects_controller_shapes_before_resolving_paths() {
    let arguments: TokenStream = quote!(routes = [UserRoute]);
    let cases = [
        quote!(
            struct Controller<T>;
        ),
        quote!(
            struct Controller(i32);
        ),
        quote!(
            #[repr(C)]
            struct Controller;
        ),
        quote!(
            #[derive(Clone)]
            struct Controller;
        ),
        quote!(
            struct Controller {
                #[derive(Clone)]
                value: i32,
            }
        ),
    ];
    for item in cases {
        let arguments = syn::parse2(arguments.clone()).expect("controller arguments should parse");
        let item = syn::parse2(item).expect("controller should parse");
        let error = expand_controller_with_common(arguments, item, &syn::parse_quote!(mads_common))
            .expect_err("controller shape must fail");
        assert!(
            error.to_string().contains("controller") || error.to_string().contains("attributes")
        );
    }
}

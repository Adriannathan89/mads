#[cfg(test)]
    mod tests {
        use super::*;
        use quote::ToTokens;

        #[test]
        fn parses_controller_route_arguments_and_rejects_duplicates() {
            let arguments: ControllerArguments =
                syn::parse_str("routes = [UserRoute, admin::AdminRoute]")
                    .expect("controller arguments should parse");
            assert_eq!(arguments.routes.len(), 2);
            assert!(syn::parse_str::<ControllerArguments>("").is_err());
            assert!(syn::parse_str::<ControllerArguments>("route = [UserRoute]").is_err());
            assert!(syn::parse_str::<ControllerArguments>("routes = []").is_err());
            assert!(
                syn::parse_str::<ControllerArguments>("routes = [UserRoute, UserRoute]").is_err()
            );
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
                        #[allow(dead_code)]
                        value: i32,
                    }
                ),
            ];
            for item in cases {
                let error =
                    expand(arguments.clone(), item).expect_err("controller shape must fail");
                assert!(
                    error.to_string().contains("controller")
                        || error.to_string().contains("attributes")
                );
            }
        }
    }

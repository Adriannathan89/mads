#[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn accepts_import_arrays() {
            let arguments: ModuleArguments = syn::parse2(quote::quote!(
                imports = [crate::users::UserModule, AuthModule]
            ))
            .expect("module imports should parse");

            let imports: Vec<_> = arguments
                .imports
                .iter()
                .map(quote::ToTokens::to_token_stream)
                .map(|import| import.to_string())
                .collect();
            assert_eq!(imports, ["crate :: users :: UserModule", "AuthModule"]);
        }

        #[test]
        fn rejects_unknown_and_repeated_module_arguments() {
            for arguments in [
                quote::quote!(providers = [Service]),
                quote::quote!(imports = [A], imports = [B]),
                quote::quote!(imports = A),
            ] {
                assert!(expand(arguments, quote::quote!(struct AppModule;)).is_err());
            }
        }

        #[test]
        fn rejects_unsupported_module_shapes_before_path_resolution() {
            let cases = [
                (
                    quote::quote!(unexpected),
                    quote::quote!(
                        struct App;
                    ),
                ),
                (
                    quote::quote!(),
                    quote::quote!(
                        struct App(i32);
                    ),
                ),
                (
                    quote::quote!(),
                    quote::quote!(
                        struct App<T>;
                    ),
                ),
            ];
            for (arguments, item) in cases {
                let error = expand(arguments, item).expect_err("module shape must fail");
                assert!(error.to_string().contains("supports only"));
            }
        }
    }

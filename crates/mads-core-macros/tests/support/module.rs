#[cfg(test)]
    mod tests {
        use super::*;

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

#[cfg(test)]
    mod tests {
        use super::*;
        use quote::ToTokens;

        fn function(source: &str) -> ItemFn {
            syn::parse_str(source).expect("test function should parse")
        }

        fn ty(source: &str) -> Type {
            syn::parse_str(source).expect("test type should parse")
        }

        #[test]
        fn validates_provider_signature_constraints() {
            let cases = [
                (
                    "fn method(&self) -> i32 { 1 }",
                    "cannot be applied to methods",
                ),
                ("fn generic<T>() -> i32 { 1 }", "does not support lifetime"),
                (
                    "fn missing() { }",
                    "requires an explicit concrete return type",
                ),
                (
                    "fn inferred() -> _ { 1 }",
                    "requires an explicit concrete return type",
                ),
                (
                    "fn opaque() -> impl Copy { 1 }",
                    "requires an explicit concrete return type",
                ),
                (
                    "unsafe fn unsafe_provider() -> i32 { 1 }",
                    "does not support unsafe",
                ),
            ];
            for (source, message) in cases {
                let error = validate_signature(&function(source)).expect_err("signature must fail");
                assert!(error.to_string().contains(message), "{source}: {error}");
            }
        }

        #[test]
        fn provider_expansion_records_the_declaration_namespace() {
            let item = function("pub fn value() -> i32 { 1 }");
            let expanded = expand_provider_with_core(item, syn::parse_quote!(mads_core))
                .expect("provider should expand")
                .to_string();

            assert!(
                expanded.contains(". with_namespace (module_path ! ())"),
                "expanded descriptor did not record its namespace: {expanded}"
            );
        }

        #[test]
        fn recognizes_fallible_output_forms() {
            for source in [
                "Result<i32>",
                "mads_core::Result<i32>",
                "mads::core::Result<i32>",
            ] {
                let return_type = ty(source);
                let output =
                    result_output(&return_type).expect("result output should be recognized");
                assert_eq!(output.to_token_stream().to_string(), "i32");
            }
        }

        #[test]
        fn rejects_non_result_or_malformed_result_types() {
            for source in [
                "i32",
                "Other::Result<i32>",
                "mads::Result<i32>",
                "Result",
                "Result<i32, String>",
                "Result<'static>",
                "<T as Trait>::Result<i32>",
            ] {
                assert!(
                    result_output(&ty(source)).is_none(),
                    "{source} must not be recognized"
                );
            }
        }

        #[test]
        fn unwraps_grouped_and_parenthesized_types() {
            assert_eq!(
                ungroup_type(&ty("(i32)")).to_token_stream().to_string(),
                "i32"
            );
            assert_eq!(
                ungroup_type(&ty("((i32))")).to_token_stream().to_string(),
                "i32"
            );
        }

        #[test]
        fn finds_inferred_types_and_const_expressions() {
            assert!(non_concrete_output_span(&ty("_")).is_some());
            assert!(non_concrete_output_span(&ty("impl Iterator<Item = i32>")).is_some());
            assert!(non_concrete_output_span(&ty("Vec<_>")).is_some());
            assert!(non_concrete_output_span(&ty("Array<{ _ }>")).is_some());
            assert!(non_concrete_output_span(&ty("Vec<i32>")).is_none());
        }

        #[test]
        fn expansion_rejects_arguments_before_resolving_paths() {
            let error = expand(
                quote!(unexpected),
                quote!(
                    fn value() -> i32 {
                        1
                    }
                ),
            )
            .expect_err("provider arguments must be rejected");
            assert!(error.to_string().contains("does not accept arguments"));
        }
    }

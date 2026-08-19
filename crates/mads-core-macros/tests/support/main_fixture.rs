#[cfg(test)]
mod tests {
    use super::*;

    fn function(source: &str) -> ItemFn {
        syn::parse_str(source).unwrap_or_else(|error| panic!("{source}: {error}"))
    }

    #[test]
    fn validates_main_signature_constraints() {
        let cases = [
            (
                "async fn not_main() {}",
                "only be applied to a function named `main`",
            ),
            ("fn main() {}", "requires an asynchronous function"),
            (
                "async fn main(value: i32) {}",
                "does not support function arguments",
            ),
            (
                "async fn main<T>() {}",
                "does not support generic parameters",
            ),
            (
                "async fn main() where T: Copy {}",
                "does not support generic parameters",
            ),
            (
                "const async fn main() {}",
                "does not support const functions",
            ),
            (
                "async unsafe fn main() {}",
                "does not support unsafe functions",
            ),
            (
                "async extern \"C\" fn main() {}",
                "does not support extern functions",
            ),
        ];
        for (source, message) in cases {
            let error =
                validate_signature(&function(source)).expect_err("main signature must fail");
            assert!(error.to_string().contains(message), "{source}: {error}");
        }
    }

    #[test]
    fn expansion_rejects_arguments_before_path_resolution() {
        let error = expand(
            quote::quote!(unexpected),
            quote::quote!(
                async fn main() {}
            ),
        )
        .expect_err("main arguments must be rejected");
        assert!(error.to_string().contains("does not accept arguments"));
    }
}

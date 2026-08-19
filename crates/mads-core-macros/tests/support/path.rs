#[cfg(test)]
    mod tests {
        use super::*;
        use quote::ToTokens;

        #[test]
        fn resolves_all_supported_crate_path_shapes() {
            assert_eq!(
                found_path(FoundCrate::Itself, false)
                    .unwrap()
                    .to_token_stream()
                    .to_string(),
                "crate"
            );
            assert_eq!(
                found_path(FoundCrate::Itself, true)
                    .unwrap()
                    .to_token_stream()
                    .to_string(),
                ":: mads :: core"
            );
            assert_eq!(
                found_path(FoundCrate::Name("my-core".into()), false)
                    .unwrap()
                    .to_token_stream()
                    .to_string(),
                ":: my_core"
            );
            assert_eq!(
                found_path(FoundCrate::Name("my-mads".into()), true)
                    .unwrap()
                    .to_token_stream()
                    .to_string(),
                ":: my_mads :: core"
            );
        }

        #[test]
        fn core_path_reports_missing_consumer_dependency() {
            let error = core_path().expect_err("the macro crate has no consumer dependency");
            assert!(error.to_string().contains("mads-core"));
        }
    }

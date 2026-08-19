#[cfg(test)]
    mod tests {
        use super::*;
        use quote::ToTokens;

        fn method(source: &str) -> TraitItemFn {
            syn::parse_str(source).unwrap_or_else(|error| panic!("{source}: {error}"))
        }

        fn literal(source: &str) -> LitStr {
            syn::parse_str(source).expect("literal should parse")
        }

        fn lit(value: &str) -> LitStr {
            LitStr::new(value, proc_macro2::Span::call_site())
        }

        #[test]
        fn parses_route_arguments() {
            assert!(matches!(
                syn::parse_str::<RoutesArguments>(""),
                Ok(RoutesArguments { prefix: None })
            ));
            let arguments: RoutesArguments = syn::parse_str("prefix = \"/api\"").unwrap();
            assert_eq!(arguments.prefix.unwrap().value(), "/api");
            assert!(syn::parse_str::<RoutesArguments>("path = \"/api\"").is_err());
            assert!(syn::parse_str::<RoutesArguments>("prefix = \"/api\" extra").is_err());
        }

        #[test]
        fn validates_trait_shapes() {
            validate_trait_shape(&syn::parse_str("trait Routes { }").unwrap()).unwrap();
            for source in [
                "trait Routes<T> { }",
                "trait Routes where Self: Sized { }",
                "unsafe trait Routes { }",
                "auto trait Routes { }",
            ] {
                assert!(
                    validate_trait_shape(&syn::parse_str(source).unwrap()).is_err(),
                    "{source}"
                );
            }
        }

        #[test]
        fn validates_each_supported_http_verb_and_rewrites_async_signature() {
            let mut routes = BTreeSet::new();
            for (verb, name) in [
                ("get", "get_route"),
                ("post", "post_route"),
                ("put", "put_route"),
                ("patch", "patch_route"),
                ("delete", "delete_route"),
            ] {
                let mut route = method(&format!("#[{verb}(\"/\")] async fn {name}(&self) -> i32;"));
                let metadata = validate_method(&mut route, &mut routes, &lit("/api")).unwrap();
                assert_eq!(metadata.full_path.value(), "/api");
                assert!(route.sig.asyncness.is_none());
                assert!(
                    route
                        .sig
                        .output
                        .to_token_stream()
                        .to_string()
                        .contains("Send")
                );
                assert_eq!(
                    metadata
                        .method
                        .tokens(&syn::parse_quote!(common))
                        .to_string(),
                    format!(
                        "common :: HttpMethod :: {}",
                        match verb {
                            "get" => "Get",
                            "post" => "Post",
                            "put" => "Put",
                            "patch" => "Patch",
                            _ => "Delete",
                        }
                    )
                );
            }
        }

        #[test]
        fn rejects_invalid_route_method_contracts() {
            let cases = [
                (
                    "#[get(\"/\")] async fn route(&self) {}",
                    "default implementations",
                ),
                ("#[get(\"/\")] fn route(&self);", "must be async"),
                (
                    "#[get(\"/\")] async unsafe fn route(&self);",
                    "cannot be const",
                ),
                ("#[get(\"/\")] async fn route();", "require `&self`"),
                (
                    "#[get(\"/\")] async fn route(&mut self);",
                    "immutable `&self`",
                ),
                (
                    "#[get(\"/\")] async fn route(self: &Self);",
                    "immutable `&self`",
                ),
                ("async fn route(&self);", "exactly one"),
                (
                    "#[get(\"/\")] #[post(\"/\")] async fn route(&self);",
                    "exactly one",
                ),
                ("#[get] async fn route(&self);", "exactly one string path"),
                (
                    "#[get(\"/\", \"/other\")] async fn route(&self);",
                    "exactly one string path",
                ),
            ];
            for (source, message) in cases {
                let mut routes = BTreeSet::new();
                let error = validate_method(&mut method(source), &mut routes, &lit("/api"))
                    .err()
                    .expect("route contract must fail");
                assert!(error.to_string().contains(message), "{source}: {error}");
            }
        }

        #[test]
        fn rejects_duplicate_verb_and_path() {
            let mut routes = BTreeSet::new();
            validate_method(
                &mut method("#[get(\"/users\")] async fn first(&self);"),
                &mut routes,
                &lit("/api"),
            )
            .unwrap();
            let error = validate_method(
                &mut method("#[get(\"/users\")] async fn second(&self);"),
                &mut routes,
                &lit("/api"),
            )
            .err()
            .expect("duplicate route should fail");
            assert!(error.to_string().contains("duplicate HTTP verb"));
        }

        #[test]
        fn validates_path_rules_and_prefix_joining() {
            for source in ["\"/\"", "\"/users/:id\"", "\"/users/_id-1\""] {
                validate_path(&literal(source), "route path", false).unwrap();
            }
            let invalid = [
                ("\"\"", false),
                ("\"users\"", false),
                ("\"/users?q=1\"", false),
                ("\"/users#fragment\"", false),
                ("\"/users\\0check\"", false),
                ("\"/users\\\\id\"", false),
                ("\"/users%20id\"", false),
                ("\"/users id\"", false),
                ("\"/users/\"", false),
                ("\"/users//id\"", false),
                ("\"/users/.\"", false),
                ("\"/users/..\"", false),
                ("\"/users/:\"", false),
                ("\"/users/:1id\"", false),
                ("\"/users/:id/:id\"", false),
                ("\"/users/id:name\"", false),
                ("\"/users/:id\"", true),
            ];
            for (source, is_prefix) in invalid {
                assert!(
                    validate_path(&literal(source), "route path", is_prefix).is_err(),
                    "{source}"
                );
            }
            assert_eq!(
                join_paths(&lit(""), &lit("/users")).unwrap().value(),
                "/users"
            );
            assert_eq!(
                join_paths(&lit("/"), &lit("/users")).unwrap().value(),
                "/users"
            );
            assert_eq!(join_paths(&lit("/api"), &lit("/")).unwrap().value(), "/api");
            assert_eq!(
                join_paths(&lit("/api"), &lit("/users")).unwrap().value(),
                "/api/users"
            );
        }

        #[test]
        fn identifies_route_verbs_and_parses_attributes() {
            let item: syn::ItemFn = syn::parse_str("#[get(\"/users\")] fn list() {}").unwrap();
            assert_eq!(route_verb(&item.attrs[0]), Some("get"));
            let unknown: syn::ItemFn = syn::parse_str("#[trace(\"/users\")] fn list() {}").unwrap();
            assert_eq!(route_verb(&unknown.attrs[0]), None);
            assert_eq!(parse_route_path(&item.attrs[0]).unwrap().value(), "/users");
        }
    }

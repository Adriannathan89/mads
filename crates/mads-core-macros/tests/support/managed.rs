#[cfg(test)]
    mod tests {
        use super::*;
        use quote::ToTokens;

        fn kind_name(kind: &ManagedKind) -> &'static str {
            kind.attribute_name()
        }

        #[test]
        fn managed_kind_metadata_is_distinct() {
            assert_eq!(kind_name(&ManagedKind::Service), "service");
            assert_eq!(kind_name(&ManagedKind::Repository), "repository");
            assert_eq!(
                ManagedKind::Service
                    .provider_kind(&syn::parse_quote!(mads))
                    .to_string(),
                "mads :: ProviderKind :: Service"
            );
            assert_eq!(
                ManagedKind::Repository
                    .provider_kind(&syn::parse_quote!(mads))
                    .to_string(),
                "mads :: ProviderKind :: Repository"
            );
            assert!(ManagedKind::Service.supported_form().contains("service"));
            assert!(
                ManagedKind::Repository
                    .supported_form()
                    .contains("repository")
            );
        }

        #[test]
        fn rejects_managed_provider_shapes_before_path_resolution() {
            for kind in [ManagedKind::Service, ManagedKind::Repository] {
                let cases = [
                    (
                        quote::quote!(unexpected),
                        quote::quote!(
                            struct Managed;
                        ),
                    ),
                    (
                        quote::quote!(),
                        quote::quote!(
                            struct Managed(i32);
                        ),
                    ),
                    (
                        quote::quote!(),
                        quote::quote!(
                            struct Managed<T>;
                        ),
                    ),
                    (
                        quote::quote!(),
                        quote::quote!(
                            #[repr(C)]
                            struct Managed;
                        ),
                    ),
                    (
                        quote::quote!(),
                        quote::quote!(
                            #[derive(Clone)]
                            struct Managed;
                        ),
                    ),
                    (
                        quote::quote!(),
                        quote::quote!(
                            struct Managed {
                                #[allow(dead_code)]
                                value: i32,
                            }
                        ),
                    ),
                ];
                for (arguments, item) in cases {
                    let error = expand(kind_ref(&kind), arguments, item)
                        .expect_err("managed provider shape must fail");
                    assert!(
                        error.to_string().contains("supports only")
                            || error.to_string().contains("attributes")
                    );
                }
            }
        }

        fn kind_ref(kind: &ManagedKind) -> ManagedKind {
            match kind {
                ManagedKind::Service => ManagedKind::Service,
                ManagedKind::Repository => ManagedKind::Repository,
            }
        }

        #[test]
        fn normalizes_self_types_inside_managed_fields() {
            let handle: Ident = syn::parse_str("Managed").unwrap();
            let mut ty: Type = syn::parse_str("Option<Self>").unwrap();
            normalize_self_type(&mut ty, &handle);
            assert_eq!(ty.to_token_stream().to_string(), "Option < Managed >");

            let mut expression: ExprPath = syn::parse_str("Self::value").unwrap();
            SelfTypeNormalizer { handle: &handle }.visit_expr_path_mut(&mut expression);
            assert_eq!(expression.to_token_stream().to_string(), "Managed :: value");
        }
    }

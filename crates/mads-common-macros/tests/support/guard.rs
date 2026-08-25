//! Unit tests for Passport guard parsing and inheritance.

use super::*;

fn parsed(source: &str) -> GuardSpec {
    syn::parse_str(source).unwrap_or_else(|error| panic!("{source}: {error}"))
}

#[test]
fn parses_the_complete_guard_grammar() {
    let spec = parsed(
        r#"strategy = "jwt", principal = ClaimsPrincipal<UserClaims>, source = cookie("access"), roles(any = ["user"]), permissions(all = ["profile:read"]), predicates = [can_read, policy::owns]"#,
    );

    assert_eq!(spec.strategy.unwrap().value(), "jwt");
    assert!(matches!(spec.source, Some(TokenSourceSpec::Cookie(_))));
    assert_eq!(spec.roles.unwrap().values.len(), 1);
    assert_eq!(spec.permissions.unwrap().values.len(), 1);
    assert_eq!(spec.predicates.unwrap().len(), 2);
}

#[test]
fn rejects_duplicate_and_mixed_skip_fields() {
    for source in [
        r#"strategy = "jwt", strategy = "other""#,
        r#"predicate = first, predicates = [second]"#,
        r#"skip, strategy = "jwt""#,
        r#"roles(any = [] )"#,
    ] {
        assert!(syn::parse_str::<GuardSpec>(source).is_err(), "{source}");
    }
}

#[test]
fn rejects_invalid_names_sources_and_policy_forms() {
    for source in [
        r#"strategy = "JWT", principal = UserPrincipal"#,
        r#"strategy = "jwt", principal = UserPrincipal, source = cookie("bad;name")"#,
        r#"strategy = "jwt", principal = UserPrincipal, source = header"#,
        r#"strategy = "jwt", principal = UserPrincipal, roles(one = ["user"])"#,
        r#"strategy = "jwt", principal = UserPrincipal, unknown = "value""#,
    ] {
        assert!(syn::parse_str::<GuardSpec>(source).is_err(), "{source}");
    }
}

#[test]
fn trait_guards_require_both_strategy_and_principal() {
    let missing_principal = parsed(r#"strategy = "jwt""#);
    assert!(validate_trait_guard(&missing_principal, Span::call_site()).is_err());

    let missing_strategy = parsed("principal = UserPrincipal");
    assert!(validate_trait_guard(&missing_strategy, Span::call_site()).is_err());
}

#[test]
fn method_fields_replace_only_their_matching_trait_fields() {
    let inherited = parsed(
        r#"strategy = "jwt", principal = UserPrincipal, source = bearer, roles(any = ["user"]), permissions(all = ["base"]), predicate = inherited"#,
    );
    let method = parsed(r#"permissions(any = ["read"]), predicates = [replacement]"#);
    validate_trait_guard(&inherited, Span::call_site()).unwrap();

    let effective = merge(Some(&inherited), Some(&method), Span::call_site())
        .unwrap()
        .expect("a method guard should remain effective");
    assert_eq!(effective.strategy.value(), "jwt");
    assert!(matches!(effective.source, TokenSourceSpec::Bearer));
    assert_eq!(effective.roles.unwrap().values[0].value(), "user");
    assert_eq!(effective.permissions.unwrap().values[0].value(), "read");
    assert_eq!(effective.predicates.len(), 1);
}

#[test]
fn skip_requires_an_inherited_trait_guard() {
    let skip = parsed("skip");
    let error = match merge(None, Some(&skip), Span::call_site()) {
        Err(error) => error,
        Ok(_) => panic!("a standalone skip must be rejected"),
    };
    assert!(error.to_string().contains("requires a guard"));
}

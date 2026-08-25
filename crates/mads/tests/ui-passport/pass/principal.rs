use std::collections::BTreeSet;

use mads::common::PassportPrincipal;

#[derive(PassportPrincipal)]
struct Principal {
    #[roles]
    roles: Vec<String>,
    #[permissions]
    permissions: BTreeSet<&'static str>,
}

#[derive(PassportPrincipal)]
struct NoPolicies {
    id: u64,
}

fn main() {
    let principal = Principal {
        roles: vec!["admin".into()],
        permissions: ["profile:read"].into_iter().collect(),
    };
    assert!(principal.has_role("admin"));
    assert!(principal.has_permission("profile:read"));

    let no_policies = NoPolicies { id: 7 };
    assert!(!no_policies.has_role("admin"));
    assert!(!no_policies.has_permission("profile:read"));
    assert_eq!(no_policies.id, 7);
}

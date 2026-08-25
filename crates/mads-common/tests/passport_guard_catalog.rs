//! Effective Passport guard metadata emitted by route contracts.

#![cfg(all(feature = "http", feature = "jwt"))]

use std::any::TypeId;

use mads_common::{
    GuardCatalog, GuardDescriptor, MADS131, PassportPrincipal, PolicyMode, TokenSource,
    core::SourceLocation,
};

struct UserPrincipal {
    user_id: u64,
}

impl PassportPrincipal for UserPrincipal {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

fn owns_profile(principal: &UserPrincipal) -> bool {
    principal.user_id == 7
}

#[mads_common::routes(prefix = "/users")]
#[mads_common::guard(
    strategy = "jwt",
    principal = UserPrincipal,
    source = bearer,
    roles(any = ["user", "admin"]),
    permissions(all = ["profile:base"]),
)]
#[allow(dead_code)]
trait UserRoutes {
    #[mads_common::get("/profile")]
    #[mads_common::guard(
        permissions(all = ["profile:read"]),
        predicate = owns_profile,
    )]
    async fn profile(&self);

    #[mads_common::post("/login")]
    #[mads_common::guard(skip)]
    async fn login(&self);
}

struct UserController;

impl UserRoutes for UserController {
    async fn profile(&self) {}

    async fn login(&self) {}
}

const MISSING_PRINCIPAL: GuardDescriptor = GuardDescriptor::new(
    "ManualRoutes",
    "profile",
    "jwt",
    None,
    None,
    TokenSource::Bearer,
    None,
    None,
    &[],
    SourceLocation::new("tests/passport_guard_catalog.rs", 1, 1),
    None,
);

#[test]
fn method_guard_merges_the_effective_policy_and_skip_omits_metadata() {
    let guards = GuardCatalog::guards();
    let profile = guards
        .into_iter()
        .find(|guard| guard.route_trait() == "UserRoutes" && guard.handler() == "profile")
        .expect("the inherited profile guard should be registered");

    assert_eq!(profile.strategy(), "jwt");
    assert_eq!(
        profile.principal_type_id(),
        Some(TypeId::of::<UserPrincipal>())
    );
    assert_eq!(
        profile.principal_type_name(),
        Some(std::any::type_name::<UserPrincipal>())
    );
    assert_eq!(profile.source(), TokenSource::Bearer);

    let roles = profile.roles().expect("inherited roles");
    assert_eq!(roles.mode(), PolicyMode::Any);
    assert_eq!(roles.values(), ["user", "admin"]);

    let permissions = profile.permissions().expect("method permissions");
    assert_eq!(permissions.mode(), PolicyMode::All);
    assert_eq!(permissions.values(), ["profile:read"]);
    assert_eq!(profile.predicates().len(), 1);
    assert!(GuardCatalog::validate().is_ok());

    let route = <UserController as UserRoutes>::__MADS_ROUTE_METADATA[0];
    assert!(std::ptr::eq(
        route.guard().expect("the route should retain its guard"),
        profile,
    ));

    assert!(
        GuardCatalog::guards()
            .into_iter()
            .all(|guard| !(guard.route_trait() == "UserRoutes" && guard.handler() == "login"))
    );
}

#[test]
fn manual_guard_metadata_without_a_principal_fails_closed() {
    let error = GuardCatalog::validate_descriptors(&[&MISSING_PRINCIPAL])
        .expect_err("manual descriptors must be fully typed");
    assert_eq!(error.code(), MADS131);
}

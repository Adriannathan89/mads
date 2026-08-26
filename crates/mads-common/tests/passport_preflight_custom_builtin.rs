//! Custom `jwt` strategies override otherwise eligible built-in adapters.

#![cfg(all(feature = "http", feature = "jwt"))]

use mads_common::{
    ClaimsPrincipal, GuardCatalog, JwtClaims, JwtTokenKind, PassportContext, PassportPrincipal,
    PassportResult, PassportStrategy, PassportStrategyCatalog,
};

#[derive(serde::Deserialize)]
struct UserClaims;

impl PassportPrincipal for UserClaims {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

#[mads_core::service]
struct CustomJwtStrategy;

#[mads_common::passport_strategy(name = "jwt")]
impl PassportStrategy for CustomJwtStrategy {
    type Claims = UserClaims;
    type Principal = ClaimsPrincipal<UserClaims>;

    const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

    async fn validate(
        &self,
        _context: &PassportContext<'_>,
        _claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal> {
        unreachable!("preflight never executes a Passport strategy")
    }
}

#[mads_common::routes]
#[mads_common::guard(strategy = "jwt", principal = ClaimsPrincipal<UserClaims>)]
#[allow(dead_code)]
trait UserRoutes {
    #[mads_common::get("/profile")]
    async fn profile(&self);
}

#[test]
fn custom_jwt_strategy_overrides_an_eligible_claims_principal_adapter() {
    let guards = GuardCatalog::guards();
    let preflight = PassportStrategyCatalog::preflight(&guards).unwrap();
    let binding = preflight.bindings().first().unwrap();

    assert!(binding.guard().builtin_adapter().is_some());
    assert_eq!(binding.strategy(), "jwt");
    assert_eq!(binding.token_kind(), JwtTokenKind::Access);
    assert!(!binding.is_builtin());
}

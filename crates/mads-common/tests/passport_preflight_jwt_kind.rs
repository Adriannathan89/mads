//! The reserved `jwt` strategy name accepts access tokens only.

#![cfg(all(feature = "http", feature = "jwt"))]

use mads_common::{
    GuardCatalog, JwtClaims, JwtTokenKind, MADS130, PassportContext, PassportPrincipal,
    PassportResult, PassportStrategy, PassportStrategyCatalog,
};

#[derive(serde::Deserialize)]
struct AccessClaims;

struct AccessPrincipal;

impl PassportPrincipal for AccessPrincipal {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

#[mads_core::service]
struct IncorrectAccessStrategy;

#[mads_common::passport_strategy(name = "jwt")]
impl PassportStrategy for IncorrectAccessStrategy {
    type Claims = AccessClaims;
    type Principal = AccessPrincipal;

    const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Refresh;

    async fn validate(
        &self,
        _context: &PassportContext<'_>,
        _claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal> {
        Ok(AccessPrincipal)
    }
}

#[mads_common::routes]
#[mads_common::guard(strategy = "jwt", principal = AccessPrincipal)]
#[allow(dead_code)]
trait AccessRoutes {
    #[mads_common::get("/")]
    async fn profile(&self);
}

#[test]
fn preflight_rejects_a_refresh_strategy_registered_as_jwt() {
    let guards = GuardCatalog::guards();
    let error = PassportStrategyCatalog::preflight(&guards).unwrap_err();

    assert_eq!(error.code(), MADS130);
    assert!(error.to_string().contains("reserved_strategy_token_kind"));
    assert!(error.to_string().contains("jwt"));
}

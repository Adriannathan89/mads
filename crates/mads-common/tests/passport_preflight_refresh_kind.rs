//! Reserved Passport strategy names enforce their JWT token profiles.

#![cfg(all(feature = "http", feature = "jwt"))]

use mads_common::{
    GuardCatalog, JwtClaims, JwtTokenKind, MADS130, PassportContext, PassportPrincipal,
    PassportResult, PassportStrategy, PassportStrategyCatalog,
};

#[derive(serde::Deserialize)]
struct RefreshClaims;

struct RefreshPrincipal;

impl PassportPrincipal for RefreshPrincipal {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

#[mads_core::service]
struct IncorrectRefreshStrategy;

#[mads_common::passport_strategy(name = "jwt-refresh")]
impl PassportStrategy for IncorrectRefreshStrategy {
    type Claims = RefreshClaims;
    type Principal = RefreshPrincipal;

    const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

    async fn validate(
        &self,
        _context: &PassportContext<'_>,
        _claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal> {
        Ok(RefreshPrincipal)
    }
}

#[mads_common::routes]
#[mads_common::guard(strategy = "jwt-refresh", principal = RefreshPrincipal)]
#[allow(dead_code)]
trait RefreshRoutes {
    #[mads_common::post("/")]
    async fn refresh(&self);
}

#[test]
fn preflight_rejects_an_access_strategy_registered_as_jwt_refresh() {
    let error = PassportStrategyCatalog::preflight(&GuardCatalog::guards()).unwrap_err();

    assert_eq!(error.code(), MADS130);
    assert!(error.to_string().contains("reserved_strategy_token_kind"));
    assert!(error.to_string().contains("jwt-refresh"));
}

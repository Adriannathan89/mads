//! Passport strategies must be registered managed providers.

#![cfg(all(feature = "http", feature = "jwt"))]

use mads_common::{
    JwtClaims, JwtTokenKind, MADS130, PassportContext, PassportPrincipal, PassportResult,
    PassportStrategy, core::Mads,
};

#[derive(serde::Deserialize)]
struct Claims;

struct Principal;

impl PassportPrincipal for Principal {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

struct UnmanagedStrategy;

#[mads_common::passport_strategy(name = "jwt")]
impl PassportStrategy for UnmanagedStrategy {
    type Claims = Claims;
    type Principal = Principal;

    const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

    async fn validate(
        &self,
        _context: &PassportContext<'_>,
        _claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal> {
        Ok(Principal)
    }
}

#[mads_common::routes]
#[mads_common::guard(strategy = "jwt", principal = Principal)]
#[allow(dead_code)]
trait ProtectedRoutes {
    #[mads_common::get("/")]
    async fn profile(&self);
}

#[test]
fn unmanaged_strategy_is_rejected_during_analysis() {
    let analysis = Mads::builder().analyze();

    assert!(!analysis.is_valid());
    assert_eq!(analysis.diagnostics()[0].code(), MADS130);
    assert!(
        analysis.diagnostics()[0]
            .to_string()
            .contains("unmanaged_strategy")
    );
    assert!(
        analysis.diagnostics()[0]
            .to_string()
            .contains("UnmanagedStrategy")
    );
}

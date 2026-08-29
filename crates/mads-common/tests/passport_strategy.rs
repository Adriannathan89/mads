//! Managed Passport strategy registration contracts.

#![cfg(all(feature = "http", feature = "jwt"))]

use std::any::TypeId;

use mads_common::core::{ApplicationContext, SourceLocation};
use mads_common::{
    GuardDescriptor, JwtClaims, JwtTokenKind, PassportContext, PassportPrincipal, PassportResult,
    PassportStrategy, PassportStrategyCatalog, PassportStrategyDescriptor, PassportStrategyFuture,
    TokenSource,
};

#[derive(serde::Deserialize)]
struct UserClaims {
    user_id: u64,
}

struct UserPrincipal;

impl From<u64> for UserPrincipal {
    fn from(_value: u64) -> Self {
        Self
    }
}

impl PassportPrincipal for UserPrincipal {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

struct ManualStrategy;

fn manual_strategy_type_id() -> TypeId {
    TypeId::of::<ManualStrategy>()
}

fn manual_strategy_type_name() -> &'static str {
    std::any::type_name::<ManualStrategy>()
}

fn user_claims_type_id() -> TypeId {
    TypeId::of::<UserClaims>()
}

fn user_claims_type_name() -> &'static str {
    std::any::type_name::<UserClaims>()
}

fn user_principal_type_id() -> TypeId {
    TypeId::of::<UserPrincipal>()
}

fn user_principal_type_name() -> &'static str {
    std::any::type_name::<UserPrincipal>()
}

fn unreachable_adapter<'a>(
    _: &'a ApplicationContext,
    _: &'a PassportContext<'a>,
    _: &'a str,
) -> PassportStrategyFuture<'a> {
    Box::pin(async { panic!("the metadata-only adapter must not be invoked") })
}

#[mads_core::service]
struct UserLookup;

#[mads_core::service]
struct AppJwtStrategy {
    users: UserLookup,
}

#[mads_common::passport_strategy(name = "jwt")]
impl PassportStrategy for AppJwtStrategy {
    type Claims = UserClaims;
    type Principal = UserPrincipal;

    const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

    async fn validate(
        &self,
        _context: &PassportContext<'_>,
        claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal> {
        let _ = &self.users;
        Ok(UserPrincipal::from(claims.custom.user_id))
    }
}

#[mads_core::service]
struct AlphaStrategy;

#[mads_common::passport_strategy(name = "alpha")]
impl PassportStrategy for AlphaStrategy {
    type Claims = UserClaims;
    type Principal = UserPrincipal;

    const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Refresh;

    async fn validate(
        &self,
        _context: &PassportContext<'_>,
        _claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal> {
        Ok(UserPrincipal)
    }
}

#[test]
fn descriptor_retains_static_strategy_types_and_kind() {
    let descriptor = PassportStrategyCatalog::strategies()
        .into_iter()
        .find(|entry| entry.name() == "jwt")
        .expect("the managed JWT strategy should be registered");

    assert_eq!(descriptor.token_kind(), JwtTokenKind::Access);
    assert_eq!(
        descriptor.provider_type_id(),
        TypeId::of::<AppJwtStrategy>()
    );
    assert_eq!(
        descriptor.principal_type_id(),
        TypeId::of::<UserPrincipal>()
    );
    assert_eq!(descriptor.claims_type_id(), TypeId::of::<UserClaims>());
}

#[test]
fn catalog_orders_strategies_by_name() {
    let names = PassportStrategyCatalog::strategies()
        .into_iter()
        .map(|descriptor| descriptor.name())
        .collect::<Vec<_>>();

    assert_eq!(names, ["alpha", "jwt"]);
}

#[test]
fn guard_and_strategy_descriptors_retain_optional_declaration_namespaces() {
    let guard = GuardDescriptor::new(
        "HealthRoutes",
        "health",
        "jwt",
        Some(user_principal_type_id),
        Some(user_principal_type_name),
        TokenSource::Bearer,
        None,
        None,
        &[],
        SourceLocation::new("guard.rs", 1, 1),
        None,
    );
    assert_eq!(guard.namespace(), None);
    assert_eq!(
        guard.with_namespace("delivery::health").namespace(),
        Some("delivery::health")
    );

    let strategy = PassportStrategyDescriptor::new(
        "manual",
        manual_strategy_type_id,
        manual_strategy_type_name,
        user_claims_type_id,
        user_claims_type_name,
        user_principal_type_id,
        user_principal_type_name,
        JwtTokenKind::Access,
        SourceLocation::new("strategy.rs", 1, 1),
        unreachable_adapter,
    );
    assert_eq!(strategy.namespace(), None);
    assert_eq!(
        strategy.with_namespace("delivery::health").namespace(),
        Some("delivery::health")
    );
}

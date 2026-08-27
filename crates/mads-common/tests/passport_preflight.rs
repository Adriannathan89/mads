//! Managed Passport strategy preflight contracts.

#![cfg(all(feature = "http", feature = "jwt"))]

use std::any::TypeId;
use std::sync::atomic::{AtomicUsize, Ordering};

use mads_common::{
    GuardCatalog, GuardDescriptor, JwtClaims, JwtTokenKind, MADS121, MADS130, PassportContext,
    PassportPrincipal, PassportResult, PassportStrategy, PassportStrategyCatalog, TokenSource,
    core::{Config, ConfigBuilder, LifecycleFuture, LifecycleHook, Mads, SourceLocation},
};

#[derive(serde::Deserialize)]
struct UserClaims {
    user_id: u64,
}

struct UserPrincipal;

impl PassportPrincipal for UserPrincipal {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

struct RefreshPrincipal;

impl PassportPrincipal for RefreshPrincipal {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

struct MismatchedPrincipal;

impl PassportPrincipal for MismatchedPrincipal {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

#[mads_core::service]
struct AccessStrategy;

#[mads_common::passport_strategy(name = "jwt")]
impl PassportStrategy for AccessStrategy {
    type Claims = UserClaims;
    type Principal = UserPrincipal;

    const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

    async fn validate(
        &self,
        _context: &PassportContext<'_>,
        claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal> {
        let _ = claims.custom.user_id;
        Ok(UserPrincipal)
    }
}

#[mads_core::service]
struct RefreshStrategy;

#[mads_common::passport_strategy(name = "jwt-refresh")]
impl PassportStrategy for RefreshStrategy {
    type Claims = UserClaims;
    type Principal = RefreshPrincipal;

    const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Refresh;

    async fn validate(
        &self,
        _context: &PassportContext<'_>,
        _claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal> {
        Ok(RefreshPrincipal)
    }
}

#[mads_common::routes(prefix = "/access")]
#[mads_common::guard(strategy = "jwt", principal = UserPrincipal)]
#[allow(dead_code)]
trait AccessRoutes {
    #[mads_common::get("/")]
    async fn profile(&self);
}

#[mads_common::routes(prefix = "/refresh")]
#[mads_common::guard(strategy = "jwt-refresh", principal = RefreshPrincipal)]
#[allow(dead_code)]
trait RefreshRoutes {
    #[mads_common::post("/")]
    async fn refresh(&self);
}

fn mismatched_principal_type_id() -> TypeId {
    TypeId::of::<MismatchedPrincipal>()
}

fn mismatched_principal_type_name() -> &'static str {
    std::any::type_name::<MismatchedPrincipal>()
}

const MISMATCHED_GUARD: GuardDescriptor = GuardDescriptor::new(
    "ManualRoutes",
    "mismatched",
    "jwt",
    Some(mismatched_principal_type_id),
    Some(mismatched_principal_type_name),
    TokenSource::Bearer,
    None,
    None,
    &[],
    SourceLocation::new("tests/passport_preflight.rs", 1, 1),
    None,
)
.with_requirement_subject("ManualRoutes::mismatched");

static ORDINARY_CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);
static LIFECYCLE_STARTS: AtomicUsize = AtomicUsize::new(0);

struct OrdinaryProvider;

#[mads_core::provider]
fn ordinary_provider() -> OrdinaryProvider {
    ORDINARY_CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
    OrdinaryProvider
}

struct CountingHook;

impl LifecycleHook for CountingHook {
    fn name(&self) -> &str {
        "preflight-counter"
    }

    fn start<'a>(&'a self, _: &'a mads_core::ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            LIFECYCLE_STARTS.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }

    fn stop<'a>(&'a self, _: &'a mads_core::ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async { Ok(()) })
    }
}

fn config() -> Config {
    ConfigBuilder::new()
        .source(mads_core::MapSource::new(
            "mads.toml",
            [("passport.secret", "01234567890123456789012345678901")],
        ))
        .build()
        .unwrap()
}

#[test]
fn managed_custom_strategies_override_jwt_and_select_refresh_independently() {
    let guards = GuardCatalog::guards();
    let preflight = PassportStrategyCatalog::preflight(&guards).unwrap();
    let access = preflight
        .bindings()
        .iter()
        .find(|binding| binding.guard().requirement_subject() == "AccessRoutes::profile")
        .unwrap();
    let refresh = preflight
        .bindings()
        .iter()
        .find(|binding| binding.guard().requirement_subject() == "RefreshRoutes::refresh")
        .unwrap();

    assert_eq!(access.strategy(), "jwt");
    assert_eq!(access.token_kind(), JwtTokenKind::Access);
    assert!(!access.is_builtin());
    assert_eq!(refresh.strategy(), "jwt-refresh");
    assert_eq!(refresh.token_kind(), JwtTokenKind::Refresh);
    assert!(!refresh.is_builtin());

    assert!(Mads::builder_with_config(config()).analyze().is_valid());
}

#[test]
fn preflight_rejects_a_guard_with_a_different_selected_principal_type() {
    let error = PassportStrategyCatalog::preflight(&[&MISMATCHED_GUARD]).unwrap_err();

    assert_eq!(error.code(), MADS130);
    assert!(error.to_string().contains("principal_mismatch"));
    assert!(error.to_string().contains("ManualRoutes::mismatched"));
}

#[tokio::test]
async fn missing_jwt_configuration_fails_before_provider_construction_or_lifecycle() {
    ORDINARY_CONSTRUCTIONS.store(0, Ordering::SeqCst);
    LIFECYCLE_STARTS.store(0, Ordering::SeqCst);
    let mut builder = Mads::builder();
    builder.lifecycle_hook(CountingHook);

    let error = match builder.build().await {
        Ok(_) => panic!("missing JWT configuration must fail before construction"),
        Err(error) => error,
    };

    assert_eq!(error.code(), MADS121);
    assert_eq!(ORDINARY_CONSTRUCTIONS.load(Ordering::SeqCst), 0);
    assert_eq!(LIFECYCLE_STARTS.load(Ordering::SeqCst), 0);
}

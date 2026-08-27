//! Native Axum Passport guard behavior.

#![cfg(all(feature = "http", feature = "jwt", feature = "cookies"))]

use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{AUTHORIZATION, WWW_AUTHENTICATE},
    },
    routing::get,
};
use mads_common::{
    Authenticated, ClaimsPrincipal, JwtClaims, JwtService, JwtSignOptions, JwtTokenKind, MADS131,
    PassportContext, PassportGuard, PassportPrincipal, PassportResult, PassportStrategy,
    TokenSource, VerifiedToken,
    core::{ApplicationContext, Config, ConfigBuilder, Mads, MapSource, ProviderRegistry},
    passport_strategy,
};
use tower::ServiceExt;

static NATIVE_HANDLER_CALLS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct NativeClaims {
    user_id: u64,
    role: String,
    permission: String,
    owns_profile: bool,
}

struct NativePrincipal(NativeClaims);

impl PassportPrincipal for NativeClaims {
    fn has_role(&self, role: &str) -> bool {
        self.role == role
    }

    fn has_permission(&self, permission: &str) -> bool {
        self.permission == permission
    }
}

impl PassportPrincipal for NativePrincipal {
    fn has_role(&self, role: &str) -> bool {
        self.0.has_role(role)
    }

    fn has_permission(&self, permission: &str) -> bool {
        self.0.has_permission(permission)
    }
}

#[mads_core::service]
struct NativeStrategy;

#[passport_strategy(name = "native")]
impl PassportStrategy for NativeStrategy {
    type Claims = NativeClaims;
    type Principal = NativePrincipal;

    const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

    async fn validate(
        &self,
        _context: &PassportContext<'_>,
        claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal> {
        Ok(NativePrincipal(claims.custom.clone()))
    }
}

fn owns_profile(principal: &NativePrincipal) -> bool {
    principal.0.owns_profile
}

fn config() -> Config {
    ConfigBuilder::new()
        .source(MapSource::new(
            "mads.toml",
            [("passport.secret", "01234567890123456789012345678901")],
        ))
        .build()
        .unwrap()
}

async fn application() -> Mads {
    let jwt = JwtService::from_config(&config()).unwrap();
    let mut builder = Mads::builder();
    builder.provide(jwt).unwrap();
    builder.build().await.unwrap()
}

async fn native_handler(
    principal: Authenticated<NativePrincipal>,
    token: VerifiedToken<NativeClaims>,
) -> String {
    NATIVE_HANDLER_CALLS.fetch_add(1, Ordering::SeqCst);
    format!("{}:{}", principal.0.user_id, token.claims.custom.user_id)
}

fn guard(application: &Mads) -> PassportGuard<NativePrincipal> {
    PassportGuard::<NativePrincipal>::builder(application.context().clone())
        .strategy("native")
        .source(TokenSource::Bearer)
        .roles_any(["user"])
        .permissions_all(["profile:read"])
        .predicate(owns_profile)
        .build()
        .unwrap()
}

async fn request(router: &Router, token: Option<String>) -> axum::response::Response {
    let mut request = Request::builder()
        .uri("/native")
        .body(Body::empty())
        .unwrap();
    if let Some(token) = token {
        request
            .headers_mut()
            .insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
    }
    router.clone().oneshot(request).await.unwrap()
}

fn claims(role: &str, permission: &str, owns_profile: bool) -> NativeClaims {
    NativeClaims {
        user_id: 7,
        role: role.to_owned(),
        permission: permission.to_owned(),
        owns_profile,
    }
}

#[tokio::test]
async fn native_guard_matches_generated_guard_authentication_and_policy_behavior() {
    NATIVE_HANDLER_CALLS.store(0, Ordering::SeqCst);
    let application = application().await;
    let jwt = application.context().resolve::<JwtService>().unwrap();
    let router = Router::new()
        .route("/native", get(native_handler))
        .route_layer(guard(&application));

    let response = request(&router, None).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[WWW_AUTHENTICATE], "Bearer");
    assert_eq!(NATIVE_HANDLER_CALLS.load(Ordering::SeqCst), 0);

    for rejected in [
        claims("guest", "profile:read", true),
        claims("user", "profile:write", true),
        claims("user", "profile:read", false),
    ] {
        let token = jwt
            .sign(rejected, JwtSignOptions::access(Duration::from_secs(60)))
            .unwrap();
        assert_eq!(
            request(&router, Some(token)).await.status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(NATIVE_HANDLER_CALLS.load(Ordering::SeqCst), 0);
    }

    let token = jwt
        .sign(
            claims("user", "profile:read", true),
            JwtSignOptions::access(Duration::from_secs(60)),
        )
        .unwrap();
    let response = request(&router, Some(token)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(NATIVE_HANDLER_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .as_ref(),
        b"7:7"
    );
}

async fn built_in_handler(principal: Authenticated<ClaimsPrincipal<NativeClaims>>) -> String {
    principal.user_id.to_string()
}

#[tokio::test]
async fn built_in_native_guard_installs_a_typed_claims_principal() {
    let application = application().await;
    let jwt = application.context().resolve::<JwtService>().unwrap();
    let guard = PassportGuard::<ClaimsPrincipal<NativeClaims>>::jwt(application.context().clone())
        .build()
        .unwrap();
    let router = Router::new()
        .route("/builtin", get(built_in_handler))
        .route_layer(guard);
    let token = jwt
        .sign(
            claims("user", "profile:read", true),
            JwtSignOptions::access(Duration::from_secs(60)),
        )
        .unwrap();
    let response = router
        .oneshot(
            Request::builder()
                .uri("/builtin")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .as_ref(),
        b"7"
    );
}

#[test]
fn native_guard_requires_an_available_jwt_service() {
    let context = ApplicationContext::new(ProviderRegistry::new(), config());
    let error = PassportGuard::<NativePrincipal>::builder(context)
        .strategy("native")
        .build()
        .unwrap_err();

    assert_eq!(error.code(), MADS131);
    assert!(error.to_string().contains("JwtService"));
}

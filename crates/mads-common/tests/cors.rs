//! Real-request CORS behavior after final router composition.

#![cfg(feature = "http")]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{
        Method, Request, StatusCode,
        header::{
            ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS,
            ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
            ACCESS_CONTROL_EXPOSE_HEADERS, ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_HEADERS,
            ACCESS_CONTROL_REQUEST_METHOD, ORIGIN,
        },
    },
    routing::get,
};
use mads_common::{
    configure_router,
    core::{Config, ConfigBuilder, Mads, MapSource},
};
use tower::ServiceExt;

const ALLOWED_ORIGIN: &str = "https://app.example.com";
const DISALLOWED_ORIGIN: &str = "https://untrusted.example.com";

fn cors_config() -> Config {
    ConfigBuilder::new()
        .source(
            MapSource::new(
                "test",
                [
                    ("passport.secret", "01234567890123456789012345678901"),
                    ("server.cors.credentials", "true"),
                    ("server.cors.max_age_seconds", "600"),
                ],
            )
            .with_string_array("server.cors.origins", [ALLOWED_ORIGIN])
            .with_string_array("server.cors.methods", ["GET", "POST"])
            .with_string_array(
                "server.cors.allowed_headers",
                ["authorization", "content-type"],
            )
            .with_string_array("server.cors.exposed_headers", ["x-request-id"]),
        )
        .build()
        .unwrap()
}

async fn configured_router(router: Router) -> Router {
    let application = Mads::builder_with_config(cors_config())
        .build()
        .await
        .unwrap();
    configure_router(&application, router).unwrap()
}

fn origin_request(path: &str, origin: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .header(ORIGIN, origin)
        .body(Body::empty())
        .unwrap()
}

fn preflight_request(origin: &str) -> Request<Body> {
    Request::builder()
        .method(Method::OPTIONS)
        .uri("/ok")
        .header(ORIGIN, origin)
        .header(ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .header(ACCESS_CONTROL_REQUEST_HEADERS, "authorization,content-type")
        .body(Body::empty())
        .unwrap()
}

fn counted_router(calls: Arc<AtomicUsize>) -> Router {
    Router::new().route(
        "/ok",
        get(move || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { "ok" }
        }),
    )
}

#[tokio::test]
async fn allowed_preflight_returns_configured_headers_without_reaching_the_handler() {
    let calls = Arc::new(AtomicUsize::new(0));
    let router = configured_router(counted_router(Arc::clone(&calls))).await;

    let response = router
        .oneshot(preflight_request(ALLOWED_ORIGIN))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
        ALLOWED_ORIGIN
    );
    assert_eq!(response.headers()[ACCESS_CONTROL_ALLOW_METHODS], "GET,POST");
    assert_eq!(
        response.headers()[ACCESS_CONTROL_ALLOW_HEADERS],
        "authorization,content-type"
    );
    assert_eq!(response.headers()[ACCESS_CONTROL_ALLOW_CREDENTIALS], "true");
    assert_eq!(response.headers()[ACCESS_CONTROL_MAX_AGE], "600");
    assert!(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn disallowed_preflight_has_no_matching_origin_or_mads_forbidden_body() {
    let calls = Arc::new(AtomicUsize::new(0));
    let router = configured_router(counted_router(Arc::clone(&calls))).await;

    let response = router
        .oneshot(preflight_request(DISALLOWED_ORIGIN))
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::FORBIDDEN);
    assert!(!response.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN));
    assert!(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn actual_requests_reach_the_handler_but_only_allowed_origins_receive_cors_headers() {
    let calls = Arc::new(AtomicUsize::new(0));
    let router = configured_router(counted_router(Arc::clone(&calls))).await;

    let allowed = router
        .clone()
        .oneshot(origin_request("/ok", ALLOWED_ORIGIN))
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
    assert_eq!(
        allowed.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
        ALLOWED_ORIGIN
    );
    assert_eq!(allowed.headers()[ACCESS_CONTROL_ALLOW_CREDENTIALS], "true");
    assert_eq!(
        allowed.headers()[ACCESS_CONTROL_EXPOSE_HEADERS],
        "x-request-id"
    );

    let disallowed = router
        .oneshot(origin_request("/ok", DISALLOWED_ORIGIN))
        .await
        .unwrap();
    assert_eq!(disallowed.status(), StatusCode::OK);
    assert!(
        !disallowed
            .headers()
            .contains_key(ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn allowed_origins_receive_cors_headers_on_application_and_router_errors() {
    let router = configured_router(
        Router::new()
            .route("/bad-request", get(|| async { StatusCode::BAD_REQUEST }))
            .route("/unauthorized", get(|| async { StatusCode::UNAUTHORIZED }))
            .route("/forbidden", get(|| async { StatusCode::FORBIDDEN }))
            .route(
                "/internal-error",
                get(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
            ),
    )
    .await;

    for (path, expected_status) in [
        ("/bad-request", StatusCode::BAD_REQUEST),
        ("/unauthorized", StatusCode::UNAUTHORIZED),
        ("/forbidden", StatusCode::FORBIDDEN),
        ("/missing", StatusCode::NOT_FOUND),
        ("/internal-error", StatusCode::INTERNAL_SERVER_ERROR),
    ] {
        let response = router
            .clone()
            .oneshot(origin_request(path, ALLOWED_ORIGIN))
            .await
            .unwrap();
        assert_eq!(response.status(), expected_status, "{path}");
        assert_eq!(
            response.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
            ALLOWED_ORIGIN
        );
    }
}

#[cfg(feature = "jwt")]
mod passport {
    use super::*;
    use std::time::Duration;

    use mads_common::__private::enable_automatic_cors_for_test;
    use mads_common::{
        JwtClaims, JwtService, JwtSignOptions, JwtTokenKind, PassportContext, PassportPrincipal,
        PassportResult, PassportStrategy, build_router, controller, passport_strategy, routes,
    };

    static STRATEGY_CALLS: AtomicUsize = AtomicUsize::new(0);
    static HANDLER_CALLS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Clone, serde::Deserialize, serde::Serialize)]
    struct Claims {
        marker: u8,
    }

    struct Principal;

    impl PassportPrincipal for Principal {
        fn has_role(&self, _role: &str) -> bool {
            false
        }

        fn has_permission(&self, _permission: &str) -> bool {
            false
        }
    }

    #[mads_common::core::service]
    struct Strategy;

    #[passport_strategy(name = "cors")]
    impl PassportStrategy for Strategy {
        type Claims = Claims;
        type Principal = Principal;

        const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

        async fn validate(
            &self,
            _context: &PassportContext<'_>,
            _claims: &JwtClaims<Self::Claims>,
        ) -> PassportResult<Self::Principal> {
            STRATEGY_CALLS.fetch_add(1, Ordering::SeqCst);
            Ok(Principal)
        }
    }

    #[routes]
    #[mads_common::guard(strategy = "cors", principal = Principal)]
    trait GuardedRoutes {
        #[mads_common::get("/guarded")]
        async fn guarded(&self) -> &'static str;
    }

    #[controller(routes = [GuardedRoutes])]
    struct GuardedController;

    impl GuardedRoutes for GuardedController {
        async fn guarded(&self) -> &'static str {
            HANDLER_CALLS.fetch_add(1, Ordering::SeqCst);
            "guarded"
        }
    }

    #[mads_common::core::module]
    struct GuardedApplication;

    #[tokio::test]
    async fn preflight_bypasses_passport_guard_and_strategy_before_actual_requests_run_them() {
        STRATEGY_CALLS.store(0, Ordering::SeqCst);
        HANDLER_CALLS.store(0, Ordering::SeqCst);

        let mut builder = Mads::builder_with_config(cors_config());
        builder.root::<GuardedApplication>().unwrap();
        assert!(enable_automatic_cors_for_test(&mut builder));
        let application = builder.build().await.unwrap();
        let jwt = application.context().resolve::<JwtService>().unwrap();
        let router = configure_router(&application, build_router(&application).unwrap()).unwrap();

        let preflight = router
            .clone()
            .oneshot(preflight_request(ALLOWED_ORIGIN))
            .await
            .unwrap();
        assert_eq!(preflight.status(), StatusCode::OK);
        assert_eq!(STRATEGY_CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(HANDLER_CALLS.load(Ordering::SeqCst), 0);

        let token = jwt
            .sign(
                Claims { marker: 1 },
                JwtSignOptions::access(Duration::from_secs(60)),
            )
            .unwrap();
        let actual = router
            .oneshot(
                Request::builder()
                    .uri("/guarded")
                    .header(ORIGIN, ALLOWED_ORIGIN)
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(actual.status(), StatusCode::OK);
        assert_eq!(
            actual.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
            ALLOWED_ORIGIN
        );
        assert_eq!(STRATEGY_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(HANDLER_CALLS.load(Ordering::SeqCst), 1);
    }
}

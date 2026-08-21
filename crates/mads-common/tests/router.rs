//! Real-request tests for validated typed router construction.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use mads_common::{build_router, controller, routes};
use mads_core::Mads;
use tower::ServiceExt;

#[routes]
trait AlphaRoutes {
    #[get("/alpha")]
    async fn lookup(&self) -> &'static str;
}

#[routes]
trait BetaRoutes {
    #[get("/beta")]
    async fn lookup(&self) -> &'static str;
}

#[controller(routes = [AlphaRoutes, BetaRoutes])]
struct LookupController;

impl AlphaRoutes for LookupController {
    async fn lookup(&self) -> &'static str {
        "alpha"
    }
}

impl BetaRoutes for LookupController {
    async fn lookup(&self) -> &'static str {
        "beta"
    }
}

#[tokio::test]
async fn typed_registrar_dispatches_same_named_trait_methods() {
    let application = Mads::builder().build().await.unwrap();
    let router = build_router(&application).unwrap();

    let alpha = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/alpha")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(alpha.status(), StatusCode::OK);
    let alpha_body = to_bytes(alpha.into_body(), usize::MAX).await.unwrap();
    assert_eq!(alpha_body.as_ref(), b"alpha");

    let beta = router
        .oneshot(Request::builder().uri("/beta").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(beta.status(), StatusCode::OK);
    let beta_body = to_bytes(beta.into_body(), usize::MAX).await.unwrap();
    assert_eq!(beta_body.as_ref(), b"beta");
}

//! Facade-level proof of same-named typed route dispatch.

use mads::common::axum::body::{Body, to_bytes};
use mads::common::axum::http::{Request, StatusCode};
use mads::common::build_router;
use mads::core::Mads;
use tower::ServiceExt;

#[mads::routes]
trait AlphaRoutes {
    #[mads::get("/alpha")]
    async fn lookup(&self) -> &'static str;
}

#[mads::routes]
trait BetaRoutes {
    #[mads::get("/beta")]
    async fn lookup(&self) -> &'static str;
}

#[mads::controller(routes = [AlphaRoutes, BetaRoutes])]
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
async fn facade_builds_a_router_with_typed_trait_dispatch() {
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

//! Real-request tests for validated typed router construction.

#![cfg(feature = "http")]

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::routing::get;
use mads_common::__private::enable_automatic_cors_for_test;
use mads_common::{
    Created, Header, HttpResult, Json, NoContent, Path, Query, build_router, configure_router,
    controller, headers, routes,
};
use mads_core::{Config, ConfigBuilder, Mads, MadsBuilder, MapSource};
use serde::{Deserialize, Serialize};
use tower::ServiceExt;

const ALLOWED_ORIGIN: &str = "https://app.example.com";

mod cors_routes {
    #[mads_common::routes]
    pub trait CorsRoutes {
        #[mads_common::get("/ok")]
        async fn ok(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [CorsRoutes])]
    pub struct CorsController;

    impl CorsRoutes for CorsController {
        async fn ok(&self) -> &'static str {
            "ok"
        }
    }

    #[mads_common::core::module]
    pub struct CorsApplication;
}

fn cors_config() -> Config {
    ConfigBuilder::new()
        .source(
            MapSource::new("test", std::iter::empty::<(&str, &str)>())
                .with_string_array("server.cors.origins", [ALLOWED_ORIGIN])
                .with_string_array("server.cors.methods", ["GET"]),
        )
        .build()
        .unwrap()
}

fn automatic_cors_builder() -> MadsBuilder {
    let mut builder = Mads::builder_with_config(cors_config());
    builder.root::<cors_routes::CorsApplication>().unwrap();
    assert!(enable_automatic_cors_for_test(&mut builder));
    builder
}

fn origin_request(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .header(header::ORIGIN, ALLOWED_ORIGIN)
        .body(Body::empty())
        .unwrap()
}

fn native_router() -> axum::Router {
    axum::Router::new().route("/native", get(|| async { "native" }))
}

#[tokio::test]
async fn raw_router_stays_unlayered_until_configured_after_native_merge() {
    let application = automatic_cors_builder().build().await.unwrap();
    let raw = build_router(&application).unwrap();

    let raw_response = raw.clone().oneshot(origin_request("/ok")).await.unwrap();
    assert!(
        !raw_response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );

    let router = configure_router(&application, raw.merge(native_router())).unwrap();
    for path in ["/ok", "/native"] {
        let response = router.clone().oneshot(origin_request(path)).await.unwrap();
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            ALLOWED_ORIGIN
        );
    }
}

#[tokio::test]
async fn raw_router_configuration_applies_cors_to_native_only_router() {
    let application = Mads::builder_with_config(cors_config())
        .build()
        .await
        .unwrap();
    let router = configure_router(&application, native_router()).unwrap();

    let response = router.oneshot(origin_request("/native")).await.unwrap();
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        ALLOWED_ORIGIN
    );
}

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

#[derive(Debug, Deserialize, Serialize)]
struct User {
    id: u64,
    name: String,
}

#[derive(Debug, Deserialize)]
struct CreateUser {
    name: String,
}

#[derive(Debug, Deserialize)]
struct PatchUser {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    page: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct RequestSummary {
    method: String,
    page: u64,
    path: String,
}

#[routes(prefix = "/users")]
trait UserRoutes {
    #[get("/:id")]
    async fn get_user(
        &self,
        id: Path<u64>,
        agent: Header<headers::UserAgent>,
    ) -> HttpResult<Json<User>>;

    #[get("/")]
    async fn list_users(
        &self,
        query: Query<ListQuery>,
        request: axum::extract::Request,
    ) -> Json<RequestSummary>;

    #[post("/")]
    async fn create_user(&self, input: Json<CreateUser>) -> Created<Json<User>>;

    #[put("/:id")]
    async fn replace_user(&self, id: Path<u64>, input: Json<CreateUser>) -> Json<User>;

    #[patch("/:id")]
    async fn patch_user(&self, id: Path<u64>, input: Json<PatchUser>) -> Json<User>;

    #[delete("/:id")]
    async fn delete_user(&self, id: Path<u64>) -> NoContent;

    #[get("/special")]
    async fn special_user(&self) -> Json<User>;
}

#[controller(routes = [UserRoutes])]
struct UserController;

impl UserRoutes for UserController {
    async fn get_user(
        &self,
        Path(id): Path<u64>,
        Header(agent): Header<headers::UserAgent>,
    ) -> HttpResult<Json<User>> {
        Ok(Json(User {
            id,
            name: agent.as_str().to_owned(),
        }))
    }

    async fn list_users(
        &self,
        Query(query): Query<ListQuery>,
        request: axum::extract::Request,
    ) -> Json<RequestSummary> {
        Json(RequestSummary {
            method: request.method().to_string(),
            page: query.page,
            path: request.uri().path().to_owned(),
        })
    }

    async fn create_user(&self, Json(input): Json<CreateUser>) -> Created<Json<User>> {
        Created(Json(User {
            id: 1,
            name: input.name,
        }))
    }

    async fn replace_user(&self, Path(id): Path<u64>, Json(input): Json<CreateUser>) -> Json<User> {
        Json(User {
            id,
            name: input.name,
        })
    }

    async fn patch_user(&self, Path(id): Path<u64>, Json(input): Json<PatchUser>) -> Json<User> {
        Json(User {
            id,
            name: input.name,
        })
    }

    async fn delete_user(&self, Path(_id): Path<u64>) -> NoContent {
        NoContent
    }

    async fn special_user(&self) -> Json<User> {
        Json(User {
            id: 999,
            name: "special".to_owned(),
        })
    }
}

async fn response_body(response: axum::response::Response) -> axum::body::Bytes {
    to_bytes(response.into_body(), usize::MAX).await.unwrap()
}

#[tokio::test]
async fn extractors_and_crud_responses_use_the_real_request() {
    let application = Mads::builder().build().await.unwrap();
    let router = build_router(&application).unwrap();

    let get = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/users/7")
                .header(header::USER_AGENT, "mads-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);
    let get = serde_json::from_slice::<User>(&response_body(get).await).unwrap();
    assert_eq!(get.id, 7);
    assert_eq!(get.name, "mads-test");

    let list = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/users?page=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list = serde_json::from_slice::<RequestSummary>(&response_body(list).await).unwrap();
    assert_eq!(list.method, Method::GET.as_str());
    assert_eq!(list.page, 2);
    assert_eq!(list.path, "/users");

    let create = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/users")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name":"created"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let create = serde_json::from_slice::<User>(&response_body(create).await).unwrap();
    assert_eq!(create.id, 1);
    assert_eq!(create.name, "created");

    for (method, body, expected_name) in [
        (Method::PUT, r#"{"name":"replaced"}"#, "replaced"),
        (Method::PATCH, r#"{"name":"patched"}"#, "patched"),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/users/7")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let user = serde_json::from_slice::<User>(&response_body(response).await).unwrap();
        assert_eq!(user.id, 7);
        assert_eq!(user.name, expected_name);
    }

    let delete = router
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/users/7")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::NO_CONTENT);
    assert!(response_body(delete).await.is_empty());
}

#[tokio::test]
async fn router_preserves_axum_http_routing_policies() {
    let application = Mads::builder().build().await.unwrap();
    let router = build_router(&application).unwrap();

    let head = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::HEAD)
                .uri("/users/7")
                .header(header::USER_AGENT, "mads-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::OK);
    assert!(response_body(head).await.is_empty());

    let options = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/users/7")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(options.status(), StatusCode::METHOD_NOT_ALLOWED);
    let allow = options
        .headers()
        .get(header::ALLOW)
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(
        allow
            .split(',')
            .map(str::trim)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from(["DELETE", "GET", "HEAD", "PATCH", "PUT"])
    );

    let special = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/users/special")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(special.status(), StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<User>(&response_body(special).await)
            .unwrap()
            .id,
        999
    );

    let missing_slash = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/users/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_slash.status(), StatusCode::NOT_FOUND);

    let missing = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let unsupported = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/users/7")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unsupported.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(unsupported.headers().contains_key(header::ALLOW));
}

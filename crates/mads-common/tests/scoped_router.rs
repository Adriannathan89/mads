//! Rooted HTTP route selection tests.

#![cfg(feature = "http")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use mads_common::core::{MADS030, Mads};
use mads_common::{build_router, controller, routes};
use tower::ServiceExt;

mod users {
    use super::shared_contracts::SharedRoutes;
    use super::*;

    #[routes]
    pub trait UserRoutes {
        #[get("/users")]
        async fn users(&self) -> &'static str;
    }

    #[controller(routes = [UserRoutes, SharedRoutes])]
    pub struct UserController;

    impl UserRoutes for UserController {
        async fn users(&self) -> &'static str {
            "users"
        }
    }

    impl SharedRoutes for UserController {
        async fn shared(&self) -> &'static str {
            "shared"
        }
    }

    #[mads_common::core::module]
    pub struct UserHttpModule;
}

mod shared_contracts {
    use super::*;

    #[routes]
    pub trait SharedRoutes {
        #[get("/shared")]
        async fn shared(&self) -> &'static str;
    }
}

mod admin {
    use super::*;

    #[routes]
    pub trait AdminRoutes {
        #[get("/admin")]
        async fn admin(&self) -> &'static str;
    }

    #[controller(routes = [AdminRoutes])]
    pub struct AdminController;

    impl AdminRoutes for AdminController {
        async fn admin(&self) -> &'static str {
            "admin"
        }
    }

    #[mads_common::core::module]
    pub struct AdminHttpModule;
}

mod duplicate_one {
    use super::*;

    #[routes]
    pub trait DuplicateOneRoutes {
        #[get("/conflict")]
        async fn conflict(&self) -> &'static str;
    }

    #[controller(routes = [DuplicateOneRoutes])]
    pub struct DuplicateOneController;

    impl DuplicateOneRoutes for DuplicateOneController {
        async fn conflict(&self) -> &'static str {
            "one"
        }
    }

    #[mads_common::core::module]
    pub struct DuplicateOneHttpModule;
}

mod duplicate_two {
    use super::*;

    #[routes]
    pub trait DuplicateTwoRoutes {
        #[get("/conflict")]
        async fn conflict(&self) -> &'static str;
    }

    #[controller(routes = [DuplicateTwoRoutes])]
    pub struct DuplicateTwoController;

    impl DuplicateTwoRoutes for DuplicateTwoController {
        async fn conflict(&self) -> &'static str {
            "two"
        }
    }

    #[mads_common::core::module]
    pub struct DuplicateTwoHttpModule;
}

mod applications {
    #[mads_common::core::module(imports = [super::users::UserHttpModule])]
    pub struct UsersApplication;

    #[mads_common::core::module(imports = [
        super::users::UserHttpModule,
        super::admin::AdminHttpModule,
    ])]
    pub struct UsersAndAdminApplication;

    #[mads_common::core::module(imports = [
        super::duplicate_one::DuplicateOneHttpModule,
        super::duplicate_two::DuplicateTwoHttpModule,
    ])]
    pub struct ConflictingApplication;
}

async fn request_status(router: mads_common::axum::Router, path: &str) -> StatusCode {
    router
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn rooted_router_installs_only_controllers_owned_by_reachable_modules() {
    let mut builder = Mads::builder();
    builder.root::<applications::UsersApplication>().unwrap();
    let application = builder.build().await.unwrap();
    let router = build_router(&application).unwrap();

    assert_eq!(
        request_status(router.clone(), "/users").await,
        StatusCode::OK
    );
    assert_eq!(
        request_status(router, "/admin").await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn rooted_router_installs_routes_from_every_reachable_http_module() {
    let mut builder = Mads::builder();
    builder
        .root::<applications::UsersAndAdminApplication>()
        .unwrap();
    let application = builder.build().await.unwrap();
    let router = build_router(&application).unwrap();

    assert_eq!(
        request_status(router.clone(), "/users").await,
        StatusCode::OK
    );
    assert_eq!(request_status(router, "/admin").await, StatusCode::OK);
}

#[tokio::test]
async fn rooted_router_inherits_unowned_route_contracts_from_selected_controllers() {
    let mut builder = Mads::builder();
    builder.root::<applications::UsersApplication>().unwrap();
    let application = builder.build().await.unwrap();
    let router = build_router(&application).unwrap();

    assert_eq!(request_status(router, "/shared").await, StatusCode::OK);
}

#[tokio::test]
async fn rooted_router_rejects_conflicting_routes_within_selected_modules() {
    let mut builder = Mads::builder();
    builder
        .root::<applications::ConflictingApplication>()
        .unwrap();
    let application = builder.build().await.unwrap();

    let error = build_router(&application).expect_err("selected route conflicts must be rejected");
    assert_eq!(error.code(), MADS030);
}

#[tokio::test]
async fn rootless_router_retains_complete_route_catalog_validation() {
    let application = Mads::builder().build().await.unwrap();

    let error = build_router(&application).expect_err("rootless builds validate every route");
    assert_eq!(error.code(), MADS030);
}

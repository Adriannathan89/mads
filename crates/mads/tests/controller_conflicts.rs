//! Integration tests for invalid controller route declarations.

use std::any::TypeId;

use mads::common::{
    ControllerRouteDescriptor, HttpMethod, RouteCatalog, RouteContractDescriptor, RouteDescriptor,
};
use mads::core::Mads;
use mads::core::{ApplicationContext, Result, SourceLocation};

struct FirstManualController;
struct SecondManualController;

fn first_manual_type_id() -> TypeId {
    TypeId::of::<FirstManualController>()
}

fn second_manual_type_id() -> TypeId {
    TypeId::of::<SecondManualController>()
}

fn no_op_registrar(
    router: mads::common::axum::Router,
    _: &ApplicationContext,
    _: &mut mads::common::__private::ValidatedRouteIter<'_>,
) -> Result<mads::common::axum::Router> {
    Ok(router)
}

const FIRST_MANUAL_ROUTE: RouteDescriptor = RouteDescriptor::new(
    HttpMethod::Get,
    "/users",
    "/:id",
    "/users/:id",
    "by_id",
    SourceLocation::new("tests/first_controller.rs", 12, 3),
);
const SECOND_MANUAL_ROUTE: RouteDescriptor = RouteDescriptor::new(
    HttpMethod::Get,
    "/users",
    "/:user_id",
    "/users/:user_id",
    "by_user_id",
    SourceLocation::new("tests/second_controller.rs", 21, 7),
);
const FIRST_MANUAL_CONTRACTS: &[RouteContractDescriptor] = &[RouteContractDescriptor::new(
    "FirstRoutes",
    &[FIRST_MANUAL_ROUTE],
)];
const SECOND_MANUAL_CONTRACTS: &[RouteContractDescriptor] = &[RouteContractDescriptor::new(
    "SecondRoutes",
    &[SECOND_MANUAL_ROUTE],
)];

#[allow(dead_code)]
#[mads::routes]
trait DuplicateReadRoutes {
    #[mads::get("/duplicate")]
    async fn first(&self);
}

#[allow(dead_code)]
#[mads::routes]
trait DuplicateAdminRoutes {
    #[mads::get("/duplicate")]
    async fn second(&self);
}

#[mads::controller(routes = [DuplicateReadRoutes, DuplicateAdminRoutes])]
struct DuplicateRouteController;

impl DuplicateReadRoutes for DuplicateRouteController {
    async fn first(&self) {}
}

impl DuplicateAdminRoutes for DuplicateRouteController {
    async fn second(&self) {}
}

#[tokio::test]
async fn controller_construction_rejects_conflicting_route_traits() {
    let mut builder = Mads::builder();
    let error = match builder.construct::<DuplicateRouteController>().await {
        Ok(_) => panic!("conflicting route traits must fail before controller allocation"),
        Err(error) => error,
    };

    assert_eq!(error.code(), mads::core::MADS030);
    assert!(error.to_string().contains("GET /duplicate"));
    assert_eq!(
        RouteCatalog::validate().unwrap_err().code(),
        mads::core::MADS030
    );
}

#[allow(dead_code)]
#[mads::routes(prefix = "/users")]
trait UserIdParameterRoutes {
    #[mads::get("/:id")]
    async fn by_id(&self);
}

#[allow(dead_code)]
#[mads::routes(prefix = "/users")]
trait UserNameParameterRoutes {
    #[mads::get("/:user_id")]
    async fn by_user_id(&self);
}

#[mads::controller(routes = [UserIdParameterRoutes, UserNameParameterRoutes])]
struct EquivalentParameterRouteController;

impl UserIdParameterRoutes for EquivalentParameterRouteController {
    async fn by_id(&self) {}
}

impl UserNameParameterRoutes for EquivalentParameterRouteController {
    async fn by_user_id(&self) {}
}

#[tokio::test]
async fn controller_construction_rejects_equivalent_parameter_route_patterns() {
    let mut builder = Mads::builder();
    let error = match builder
        .construct::<EquivalentParameterRouteController>()
        .await
    {
        Ok(_) => panic!("equivalent parameter route patterns must conflict"),
        Err(error) => error,
    };

    assert_eq!(error.code(), mads::core::MADS030);
    assert!(error.to_string().contains("GET /users/:user_id"));
}

#[test]
fn cross_controller_dynamic_conflicts_report_both_declarations() {
    let first = ControllerRouteDescriptor::with_registrar(
        "test::FirstManualController",
        first_manual_type_id,
        FIRST_MANUAL_CONTRACTS,
        no_op_registrar,
    );
    let second = ControllerRouteDescriptor::with_registrar(
        "test::SecondManualController",
        second_manual_type_id,
        SECOND_MANUAL_CONTRACTS,
        no_op_registrar,
    );

    let error = mads::common::__private::validate_descriptors(&[&first, &second])
        .expect_err("equivalent dynamic routes across controllers must conflict");

    assert_eq!(error.code(), mads::core::MADS030);
    let rendered = error.to_string();
    assert!(rendered.contains("tests/first_controller.rs:12:3"));
    assert!(rendered.contains("tests/second_controller.rs:21:7"));
}

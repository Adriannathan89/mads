//! Integration tests for invalid controller route declarations.

use std::any::TypeId;
use std::sync::atomic::{AtomicUsize, Ordering};

use mads::common::{
    ControllerRouteDescriptor, HttpMethod, RouteCatalog, RouteContractDescriptor, RouteDescriptor,
    build_router,
};
use mads::core::{ApplicationContext, MADS003, Mads, Result, SourceLocation};

static REGISTRATIONS: AtomicUsize = AtomicUsize::new(0);

struct FirstManualController;
struct SecondManualController;
struct CountedManualController;
struct MissingManualController;

fn first_manual_type_id() -> TypeId {
    TypeId::of::<FirstManualController>()
}

fn second_manual_type_id() -> TypeId {
    TypeId::of::<SecondManualController>()
}

fn counted_manual_type_id() -> TypeId {
    TypeId::of::<CountedManualController>()
}

fn missing_manual_type_id() -> TypeId {
    TypeId::of::<MissingManualController>()
}

fn no_op_registrar(
    router: mads::common::axum::Router,
    _: &ApplicationContext,
    _: &mut mads::common::__private::ValidatedRouteIter<'_>,
) -> Result<mads::common::axum::Router> {
    Ok(router)
}

fn counted_registrar(
    router: mads::common::axum::Router,
    _: &ApplicationContext,
    _: &mut mads::common::__private::ValidatedRouteIter<'_>,
) -> Result<mads::common::axum::Router> {
    REGISTRATIONS.fetch_add(1, Ordering::SeqCst);
    Ok(router)
}

fn missing_controller_registrar(
    router: mads::common::axum::Router,
    context: &ApplicationContext,
    _: &mut mads::common::__private::ValidatedRouteIter<'_>,
) -> Result<mads::common::axum::Router> {
    let _ = context.resolve::<MissingManualController>()?;
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
const COUNTED_MANUAL_ROUTE: RouteDescriptor = RouteDescriptor::new(
    HttpMethod::Get,
    "",
    "/counted",
    "/counted",
    "counted",
    SourceLocation::new("tests/counted_controller.rs", 3, 1),
);
const MISSING_MANUAL_ROUTE: RouteDescriptor = RouteDescriptor::new(
    HttpMethod::Get,
    "",
    "/missing",
    "/missing",
    "missing",
    SourceLocation::new("tests/missing_controller.rs", 3, 1),
);
const FIRST_MANUAL_CONTRACTS: &[RouteContractDescriptor] = &[RouteContractDescriptor::new(
    "FirstRoutes",
    &[FIRST_MANUAL_ROUTE],
)];
const SECOND_MANUAL_CONTRACTS: &[RouteContractDescriptor] = &[RouteContractDescriptor::new(
    "SecondRoutes",
    &[SECOND_MANUAL_ROUTE],
)];
const COUNTED_MANUAL_CONTRACTS: &[RouteContractDescriptor] = &[RouteContractDescriptor::new(
    "CountedRoutes",
    &[COUNTED_MANUAL_ROUTE],
)];
const MISSING_MANUAL_CONTRACTS: &[RouteContractDescriptor] = &[RouteContractDescriptor::new(
    "MissingRoutes",
    &[MISSING_MANUAL_ROUTE],
)];

mads::core::__private::inventory::submit! {
    ControllerRouteDescriptor::with_registrar(
        "aaa::CountedManualController",
        counted_manual_type_id,
        COUNTED_MANUAL_CONTRACTS,
        counted_registrar,
    )
}

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
async fn router_validation_rejects_conflicts_before_any_registration() {
    REGISTRATIONS.store(0, Ordering::SeqCst);
    let application = Mads::builder().build().await.unwrap();
    let error = build_router(&application).expect_err("conflicting routes must fail bootstrap");

    assert_eq!(error.code(), mads::core::MADS030);
    assert!(error.to_string().contains("GET /duplicate"));
    assert_eq!(REGISTRATIONS.load(Ordering::SeqCst), 0);
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
async fn controller_construction_remains_independent_of_http_validation() {
    let mut builder = Mads::builder();
    builder
        .construct::<EquivalentParameterRouteController>()
        .await
        .expect("metadata-only controller construction must succeed");

    let error = RouteCatalog::validate().unwrap_err();
    assert_eq!(error.code(), mads::core::MADS030);
    assert!(error.to_string().contains("GET /duplicate"));
}

#[tokio::test]
async fn missing_controller_resolution_returns_core_diagnostic() {
    let descriptor = ControllerRouteDescriptor::with_registrar(
        "test::MissingManualController",
        missing_manual_type_id,
        MISSING_MANUAL_CONTRACTS,
        missing_controller_registrar,
    );
    let mut controllers = mads::common::__private::validate_descriptors(&[&descriptor]).unwrap();
    let controller = controllers.pop().unwrap();
    let mut routes = controller.routes();
    let application = Mads::builder().build().await.unwrap();

    let error = (controller.registrar())(
        mads::common::axum::Router::new(),
        application.context(),
        &mut routes,
    )
    .expect_err("a missing controller must return its resolution diagnostic");

    assert_eq!(error.code(), MADS003);
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

//! Integration test for an unresolved generated controller.

use std::any::TypeId;

use mads::common::__private::RouterBuildContext;
use mads::common::{
    ControllerRouteDescriptor, HttpMethod, RouteContractDescriptor, RouteDescriptor, build_router,
};
use mads::core::{MADS003, Mads, Result, SourceLocation};

struct MissingManualController;

fn missing_manual_type_id() -> TypeId {
    TypeId::of::<MissingManualController>()
}

fn missing_controller_registrar(
    router: mads::common::axum::Router,
    context: &RouterBuildContext<'_>,
    _: &mut mads::common::__private::ValidatedRouteIter<'_>,
) -> Result<mads::common::axum::Router> {
    let _ = context.application().resolve::<MissingManualController>()?;
    Ok(router)
}

const MISSING_MANUAL_ROUTE: RouteDescriptor = RouteDescriptor::new(
    HttpMethod::Get,
    "",
    "/missing",
    "/missing",
    "missing",
    SourceLocation::new("tests/missing_controller.rs", 3, 1),
);
const MISSING_MANUAL_CONTRACTS: &[RouteContractDescriptor] = &[RouteContractDescriptor::new(
    "MissingRoutes",
    &[MISSING_MANUAL_ROUTE],
)];

mads::core::__private::inventory::submit! {
    ControllerRouteDescriptor::with_registrar(
        "test::MissingManualController",
        missing_manual_type_id,
        MISSING_MANUAL_CONTRACTS,
        missing_controller_registrar,
    )
}

#[tokio::test]
async fn missing_controller_resolution_returns_core_diagnostic() {
    let application = Mads::builder().build().await.unwrap();
    let error = build_router(&application)
        .expect_err("a missing controller must return its resolution diagnostic");

    assert_eq!(error.code(), MADS003);
}

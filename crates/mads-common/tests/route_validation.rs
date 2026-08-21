//! Integration tests for runtime validation of route metadata.

use std::any::TypeId;

use mads_common::core::{ApplicationContext, MADS030, Result, SourceLocation};
use mads_common::{
    ControllerRouteDescriptor, HttpMethod, RouteContractDescriptor, RouteDescriptor,
};

struct FirstController;
struct SecondController;

fn first_type_id() -> TypeId {
    TypeId::of::<FirstController>()
}

fn second_type_id() -> TypeId {
    TypeId::of::<SecondController>()
}

fn no_op_registrar(
    router: mads_common::axum::Router,
    _: &ApplicationContext,
    _: &mut mads_common::__private::ValidatedRouteIter<'_>,
) -> Result<mads_common::axum::Router> {
    Ok(router)
}

const VALID_ROUTE: RouteDescriptor = RouteDescriptor::new(
    HttpMethod::Get,
    "/users",
    "/:id",
    "/users/:id",
    "get_user",
    SourceLocation::new("tests/route_validation.rs", 10, 5),
);

const BAD_JOIN: RouteDescriptor = RouteDescriptor::new(
    HttpMethod::Get,
    "/users",
    "/:id",
    "/wrong/:id",
    "get_user",
    SourceLocation::new("tests/route_validation.rs", 20, 5),
);

const BAD_PREFIX: RouteDescriptor = RouteDescriptor::new(
    HttpMethod::Get,
    "/users/:id",
    "/posts",
    "/users/:id/posts",
    "list_posts",
    SourceLocation::new("tests/route_validation.rs", 30, 5),
);

const BAD_PATH: RouteDescriptor = RouteDescriptor::new(
    HttpMethod::Get,
    "",
    "/users/",
    "/users/",
    "list_users",
    SourceLocation::new("tests/route_validation.rs", 40, 5),
);

const BAD_LOCATION: RouteDescriptor = RouteDescriptor::new(
    HttpMethod::Get,
    "",
    "/users",
    "/users",
    "list_users",
    SourceLocation::new("", 0, 0),
);

const FIRST_ROUTES: &[RouteDescriptor] = &[VALID_ROUTE];
const FIRST_CONTRACTS: &[RouteContractDescriptor] =
    &[RouteContractDescriptor::new("UserRoutes", FIRST_ROUTES)];
const BAD_PREFIX_CONTRACTS: &[RouteContractDescriptor] =
    &[RouteContractDescriptor::new("Routes", &[BAD_PREFIX])];
const BAD_PATH_CONTRACTS: &[RouteContractDescriptor] =
    &[RouteContractDescriptor::new("Routes", &[BAD_PATH])];
const BAD_JOIN_CONTRACTS: &[RouteContractDescriptor] =
    &[RouteContractDescriptor::new("Routes", &[BAD_JOIN])];
const BAD_LOCATION_CONTRACTS: &[RouteContractDescriptor] =
    &[RouteContractDescriptor::new("Routes", &[BAD_LOCATION])];
const EMPTY_ROUTE_CONTRACTS: &[RouteContractDescriptor] =
    &[RouteContractDescriptor::new("Routes", &[])];
const UNNAMED_CONTRACTS: &[RouteContractDescriptor] =
    &[RouteContractDescriptor::new("", FIRST_ROUTES)];
const DUPLICATE_CONTRACTS: &[RouteContractDescriptor] = &[
    RouteContractDescriptor::new("Routes", FIRST_ROUTES),
    RouteContractDescriptor::new("Routes", FIRST_ROUTES),
];

fn controller(
    type_name: &'static str,
    type_id: fn() -> TypeId,
    contracts: &'static [RouteContractDescriptor],
) -> ControllerRouteDescriptor {
    ControllerRouteDescriptor::with_registrar(type_name, type_id, contracts, no_op_registrar)
}

fn assert_invalid(controllers: &[&ControllerRouteDescriptor]) {
    let error = mads_common::__private::validate_descriptors(controllers)
        .expect_err("untrusted route metadata must be rejected");
    assert_eq!(error.code(), MADS030);
}

#[test]
fn rejects_invalid_route_paths_and_source_coordinates() {
    for contracts in [
        BAD_PREFIX_CONTRACTS,
        BAD_PATH_CONTRACTS,
        BAD_JOIN_CONTRACTS,
        BAD_LOCATION_CONTRACTS,
    ] {
        let descriptor = controller("test::Controller", first_type_id, contracts);
        assert_invalid(&[&descriptor]);
    }
}

#[test]
fn rejects_invalid_controller_and_contract_metadata() {
    let empty_identity = controller("", first_type_id, FIRST_CONTRACTS);
    assert_invalid(&[&empty_identity]);

    let no_contracts = controller("test::Controller", first_type_id, &[]);
    assert_invalid(&[&no_contracts]);

    let empty_contract = controller("test::Controller", first_type_id, EMPTY_ROUTE_CONTRACTS);
    assert_invalid(&[&empty_contract]);

    let unnamed_contract = controller("test::Controller", first_type_id, UNNAMED_CONTRACTS);
    assert_invalid(&[&unnamed_contract]);

    let duplicate_contract = controller("test::Controller", first_type_id, DUPLICATE_CONTRACTS);
    assert_invalid(&[&duplicate_contract]);
}

#[test]
fn rejects_duplicate_controller_identities_and_missing_registrars() {
    let first = controller("test::First", first_type_id, FIRST_CONTRACTS);
    let duplicate_type_id = controller("test::Second", first_type_id, FIRST_CONTRACTS);
    assert_invalid(&[&first, &duplicate_type_id]);

    let duplicate_type_name = controller("test::First", second_type_id, FIRST_CONTRACTS);
    assert_invalid(&[&first, &duplicate_type_name]);

    let missing_registrar =
        ControllerRouteDescriptor::new("test::MetadataOnly", second_type_id, FIRST_CONTRACTS);
    assert_invalid(&[&missing_registrar]);
}

#[test]
fn validated_routes_translate_parameters_for_axum() {
    let descriptor = controller("test::Controller", first_type_id, FIRST_CONTRACTS);
    let controllers = mads_common::__private::validate_descriptors(&[&descriptor])
        .expect("valid metadata must produce validated routes");
    let mut routes = controllers[0].routes();

    assert_eq!(
        routes
            .next(HttpMethod::Get, "get_user")
            .expect("validated route must preserve method and handler"),
        "/users/{id}"
    );
    routes
        .finish()
        .expect("all validated routes must be consumed");
}

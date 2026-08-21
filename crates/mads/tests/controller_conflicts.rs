//! Integration tests for invalid controller route declarations.

use mads::common::RouteCatalog;
use mads::core::Mads;

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

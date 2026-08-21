//! Runtime integration tests for facade-exported managed-provider attributes.

use std::sync::Arc;

use mads::common::{HttpMethod, RouteCatalog};
use mads::core::{Catalog as CoreCatalog, Mads as CoreMads, ProviderKind, ProviderVisibility};

#[test]
fn prelude_exposes_core_types_and_bare_attributes() {
    use mads::prelude::*;

    mod declarations {
        use mads::prelude::*;

        #[module]
        struct PreludeModule;

        #[provider]
        fn prelude_value() -> usize {
            1
        }

        #[repository]
        struct PreludeRepository;

        #[service]
        struct PreludeService;

        #[allow(dead_code)]
        #[routes(prefix = "/prelude")]
        trait PreludeRoutes {
            #[get("/")]
            async fn index(&self);
        }

        #[controller(routes = [PreludeRoutes])]
        struct PreludeController;

        impl PreludeRoutes for PreludeController {
            async fn index(&self) {}
        }

        #[main]
        async fn main() {}
    }

    let _ = std::any::TypeId::of::<Mads>();
    let _ = std::any::TypeId::of::<Config>();
    let _ = std::any::TypeId::of::<Diagnostic>();
    let _ = std::any::TypeId::of::<Catalog>();
    let _ = std::any::TypeId::of::<LifecycleState>();
}

#[mads::module]
struct FacadeModule;

/// Public managed service used to verify facade visibility metadata.
#[mads::service]
pub struct PublicGraphService;

#[mads::provider]
pub(crate) fn restricted_graph_value() -> u16 {
    16
}

#[mads::repository]
struct FacadeRepository;

#[derive(Clone)]
struct Clock;

struct GroupedFallibleProvider;

#[mads::service]
struct QueryUsecase;

#[mads::service]
struct CommandUsecase;

#[mads::routes(prefix = "/users")]
trait QueryRoutes {
    #[mads::get("/:id")]
    async fn get_user(&self, id: i64) -> i64;
}

#[mads::routes]
trait CommandRoutes {
    #[mads::post("/users")]
    async fn create_user(&self, id: i64) -> i64;
}

#[mads::controller(routes = [QueryRoutes, CommandRoutes])]
struct FacadeController {
    query: QueryUsecase,
    command: CommandUsecase,
}

impl QueryRoutes for FacadeController {
    async fn get_user(&self, id: i64) -> i64 {
        let _query = &self.query;
        id
    }
}

impl CommandRoutes for FacadeController {
    async fn create_user(&self, id: i64) -> i64 {
        let _command = &self.command;
        id
    }
}

#[allow(clippy::result_large_err, unused_parens)]
#[mads::provider]
fn grouped_fallible_provider() -> (mads::core::Result<GroupedFallibleProvider>) {
    Ok(GroupedFallibleProvider)
}

#[mads::service]
struct FacadeService {
    repository: FacadeRepository,
    clock: Clock,
}

impl FacadeService {
    fn inner_address(&self) -> *const () {
        std::ptr::from_ref(&**self).cast()
    }

    fn has_dependencies(&self) -> bool {
        let _repository = &self.repository;
        let _clock = &self.clock;
        true
    }
}

#[test]
fn facade_attributes_register_stable_descriptors() {
    let module_names: Vec<_> = CoreCatalog::modules()
        .into_iter()
        .map(|descriptor| descriptor.type_name())
        .collect();
    let providers = CoreCatalog::providers();

    assert!(module_names.contains(&"facade::FacadeModule"));
    assert!(providers.iter().any(|descriptor| {
        descriptor.type_name() == "facade::FacadeRepository"
            && descriptor.kind() == ProviderKind::Repository
    }));
    assert!(providers.iter().any(|descriptor| {
        descriptor.type_name() == "facade::FacadeService"
            && descriptor.kind() == ProviderKind::Service
    }));
    assert!(providers.iter().any(|descriptor| {
        descriptor.type_name() == "facade::FacadeController"
            && descriptor.kind() == ProviderKind::Service
    }));
    assert_eq!(
        CoreCatalog::provider_for::<PublicGraphService>()
            .expect("public service descriptor should exist")
            .visibility(),
        ProviderVisibility::Public,
    );
    assert_eq!(
        CoreCatalog::provider_for::<u16>()
            .expect("restricted provider descriptor should exist")
            .visibility(),
        ProviderVisibility::Private,
    );
}

#[test]
fn controller_dependencies_follow_source_field_order() {
    let descriptor = CoreCatalog::provider_for::<FacadeController>()
        .expect("the facade controller descriptor should be registered");
    let dependency_names: Vec<_> = descriptor
        .dependencies()
        .iter()
        .map(|dependency| dependency.type_name())
        .collect();

    assert_eq!(dependency_names, ["QueryUsecase", "CommandUsecase"]);
}

#[test]
fn controller_keeps_deterministic_route_metadata() {
    let routes = RouteCatalog::routes_for::<FacadeController>();

    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].method(), HttpMethod::Get);
    assert_eq!(routes[0].prefix(), "/users");
    assert_eq!(routes[0].path(), "/:id");
    assert_eq!(routes[0].full_path(), "/users/:id");
    assert_eq!(routes[0].handler(), "get_user");
    assert_eq!(routes[1].method(), HttpMethod::Post);
    assert_eq!(routes[1].full_path(), "/users");
    assert!(RouteCatalog::validate_controller::<FacadeController>().is_ok());
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
async fn controller_construction_rejects_conflicting_route_traits() {
    let mut builder = CoreMads::builder();
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
    let mut builder = CoreMads::builder();
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

#[allow(dead_code)]
#[mads::routes(prefix = "/")]
trait RootRoutes {
    #[mads::get("/health")]
    async fn health(&self);
}

#[mads::controller(routes = [RootRoutes])]
struct RootController;

impl RootRoutes for RootController {
    async fn health(&self) {}
}

#[test]
fn root_prefix_does_not_create_an_ambiguous_double_slash_path() {
    let routes = RouteCatalog::routes_for::<RootController>();
    assert_eq!(routes[0].full_path(), "/health");
}

#[test]
fn service_dependencies_follow_source_field_order() {
    let descriptor = CoreCatalog::provider_for::<FacadeService>()
        .expect("the facade service descriptor should be registered");
    let dependency_names: Vec<_> = descriptor
        .dependencies()
        .iter()
        .map(|dependency| dependency.type_name())
        .collect();

    assert_eq!(dependency_names, ["FacadeRepository", "Clock"]);
}

#[tokio::test]
async fn cloned_service_handles_share_the_inner_allocation() {
    let mut builder = CoreMads::builder();
    builder
        .provide(Clock)
        .expect("clock insertion should succeed");
    builder
        .construct::<FacadeRepository>()
        .await
        .expect("repository construction should succeed");
    builder
        .construct::<FacadeService>()
        .await
        .expect("service construction should succeed");

    let application = builder.build();
    let service = application
        .context()
        .resolve::<FacadeService>()
        .expect("the constructed service should resolve");
    let cloned = service.as_ref().clone();

    assert!(service.has_dependencies());
    assert_eq!(service.inner_address(), cloned.inner_address());
    assert!(Arc::ptr_eq(
        &service,
        &application.context().resolve().unwrap()
    ));
}

#[tokio::test]
async fn controller_constructs_after_multiple_usecases() {
    let mut builder = CoreMads::builder();
    builder
        .construct::<QueryUsecase>()
        .await
        .expect("query use case construction should succeed");
    builder
        .construct::<CommandUsecase>()
        .await
        .expect("command use case construction should succeed");
    builder
        .construct::<FacadeController>()
        .await
        .expect("controller construction should succeed");

    let application = builder.build();
    let controller = application
        .context()
        .resolve::<FacadeController>()
        .expect("the constructed controller should resolve");
    let cloned = controller.as_ref().clone();

    assert_eq!(controller.get_user(7).await, 7);
    assert_eq!(controller.create_user(8).await, 8);
    assert!(std::ptr::eq(&**controller, &*cloned));
}

#[tokio::test]
async fn grouped_mads_results_register_their_success_type() {
    let mut builder = CoreMads::builder();

    builder
        .construct::<GroupedFallibleProvider>()
        .await
        .expect("a grouped MADS Result provider should construct its success type");

    let application = builder.build();
    assert!(
        application
            .context()
            .resolve::<GroupedFallibleProvider>()
            .is_ok()
    );
}

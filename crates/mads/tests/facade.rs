//! Runtime integration tests for facade-exported managed-provider attributes.

use std::sync::Arc;

use mads::core::{Catalog as CoreCatalog, Mads as CoreMads, ProviderKind};

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

#[mads::repository]
struct FacadeRepository;

#[derive(Clone)]
struct Clock;

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

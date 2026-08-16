//! Integration tests for application-scoped provider storage.

use std::any::TypeId;
use std::sync::Arc;

use mads_core::{
    ApplicationContext, ConfigBuilder, ConstructionContext, ErasedProvider, MADS001, MADS003,
    MADS004, MapSource, ProviderRegistry,
};

#[derive(Debug)]
struct Counter;

#[test]
fn resolves_the_same_application_scoped_allocation() {
    let mut registry = ProviderRegistry::new();
    registry
        .insert(Counter)
        .expect("first insertion should work");

    let first = registry
        .resolve::<Counter>()
        .expect("provider should resolve");
    let second = registry
        .resolve::<Counter>()
        .expect("provider should resolve");

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(registry.insert(Counter).unwrap_err().code(), MADS001);
    assert_eq!(registry.resolve::<String>().unwrap_err().code(), MADS003);
}

#[test]
fn reports_a_type_mismatch_for_invalid_erased_provider_storage() {
    let mut registry = ProviderRegistry::new();
    let value: ErasedProvider = Arc::new(String::from("not a counter"));

    registry
        .insert_erased(
            TypeId::of::<Counter>(),
            std::any::type_name::<Counter>(),
            value,
        )
        .expect("erased provider should insert");

    assert_eq!(registry.resolve::<Counter>().unwrap_err().code(), MADS004);
}

#[test]
fn contexts_resolve_shared_providers_and_expose_merged_configuration() {
    let mut registry = ProviderRegistry::new();
    registry.insert(Counter).expect("provider should insert");
    let config = ConfigBuilder::new()
        .source(MapSource::new("defaults", [("server.port", "3000")]))
        .source(MapSource::new("environment", [("server.port", "8080")]))
        .build()
        .expect("configuration should build");

    let construction = ConstructionContext::new(&registry, &config);
    let construction_provider = construction
        .resolve::<Counter>()
        .expect("construction provider should resolve");
    assert_eq!(construction.config().get("server.port"), Some("8080"));

    let application = ApplicationContext::new(registry, config);
    let clone = application.clone();
    let first = application
        .resolve::<Counter>()
        .expect("application provider should resolve");
    let second = clone
        .resolve::<Counter>()
        .expect("cloned application provider should resolve");

    assert!(Arc::ptr_eq(&construction_provider, &first));
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(application.config().get("server.port"), Some("8080"));
    assert_eq!(clone.config().get("server.port"), Some("8080"));
}

//! Integration tests for deterministic descriptor discovery.

use std::any::TypeId;
use std::sync::Arc;

use mads_core::{
    Catalog, ConstructionContext, ErasedProvider, MADS001, MADS003, ModuleDescriptor,
    ProviderDescriptor, ProviderFuture, ProviderKind, ProviderVisibility, SourceLocation,
};

struct Alpha;
struct Duplicate;
struct Missing;
struct Zeta;

fn alpha_type_id() -> TypeId {
    TypeId::of::<Alpha>()
}

fn zeta_type_id() -> TypeId {
    TypeId::of::<Zeta>()
}

fn duplicate_type_id() -> TypeId {
    TypeId::of::<Duplicate>()
}

fn alpha_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async { Ok(Arc::new(Alpha) as ErasedProvider) })
}

fn duplicate_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async { Ok(Arc::new(Duplicate) as ErasedProvider) })
}

inventory::submit! {
    ModuleDescriptor::new("zeta::Module", zeta_type_id, SourceLocation::new(file!(), line!(), column!()))
}

inventory::submit! {
    ModuleDescriptor::new("alpha::Module", alpha_type_id, SourceLocation::new(file!(), line!(), column!()))
}

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Provider,
        "alpha::Provider",
        alpha_type_id,
        &[],
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        alpha_constructor,
    )
}

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Service,
        "duplicate::First",
        duplicate_type_id,
        &[],
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        duplicate_constructor,
    )
}

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Repository,
        "duplicate::Second",
        duplicate_type_id,
        &[],
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        duplicate_constructor,
    )
}

#[test]
fn modules_are_sorted_by_stable_name() {
    let names: Vec<_> = Catalog::modules()
        .into_iter()
        .map(|item| item.type_name())
        .collect();

    assert_eq!(names, ["alpha::Module", "zeta::Module"]);
}

#[test]
fn provider_for_selects_the_matching_descriptor() {
    let descriptor = Catalog::provider_for::<Alpha>().expect("alpha provider should be selected");

    assert_eq!(descriptor.type_name(), "alpha::Provider");
    assert_eq!(descriptor.type_id(), TypeId::of::<Alpha>());
}

#[test]
fn provider_for_reports_duplicate_descriptors() {
    let Err(error) = Catalog::provider_for::<Duplicate>() else {
        panic!("duplicates should be rejected");
    };

    assert_eq!(error.code(), MADS001);
}

#[test]
fn provider_for_reports_a_missing_descriptor() {
    let Err(error) = Catalog::provider_for::<Missing>() else {
        panic!("missing providers should be rejected");
    };

    assert_eq!(error.code(), MADS003);
}

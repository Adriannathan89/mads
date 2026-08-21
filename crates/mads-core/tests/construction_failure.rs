//! Integration tests for provider-construction failure diagnostics.

use std::any::TypeId;
use std::sync::Arc;

use mads_core::{
    ConstructionContext, DependencyDescriptor, Diagnostic, ErasedProvider, Error, MADS006, MADS020,
    Mads, ProviderDescriptor, ProviderFuture, ProviderKind, ProviderVisibility, SourceLocation,
};

#[derive(Debug)]
struct ConstructorCause;

impl std::fmt::Display for ConstructorCause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("database refused startup")
    }
}

impl std::error::Error for ConstructorCause {}

struct FailingDatabase;
struct FailingRepository;
struct TopLevelService;

fn database_type_id() -> TypeId {
    TypeId::of::<FailingDatabase>()
}

fn repository_type_id() -> TypeId {
    TypeId::of::<FailingRepository>()
}

fn service_type_id() -> TypeId {
    TypeId::of::<TopLevelService>()
}

fn database_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async {
        Err(Error::with_source(
            Diagnostic::new(MADS020, "test constructor failed", "fixture failure"),
            ConstructorCause,
        ))
    })
}

fn repository_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async { Ok(Arc::new(FailingRepository) as ErasedProvider) })
}

fn service_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async { Ok(Arc::new(TopLevelService) as ErasedProvider) })
}

static REPOSITORY_DEPENDENCIES: [DependencyDescriptor; 1] = [DependencyDescriptor::new(
    "FailingDatabase",
    database_type_id,
)];
static SERVICE_DEPENDENCIES: [DependencyDescriptor; 1] = [DependencyDescriptor::new(
    "FailingRepository",
    repository_type_id,
)];

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Provider,
        "failure::FailingDatabase",
        database_type_id,
        &[],
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        database_constructor,
    )
}

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Repository,
        "failure::FailingRepository",
        repository_type_id,
        &REPOSITORY_DEPENDENCIES,
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        repository_constructor,
    )
}

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Service,
        "failure::TopLevelService",
        service_type_id,
        &SERVICE_DEPENDENCIES,
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        service_constructor,
    )
}

#[tokio::test]
async fn construction_failure_preserves_source_and_consumer_path() {
    let Err(error) = Mads::builder().build().await else {
        panic!("the failing provider must abort construction");
    };

    assert_eq!(error.code(), MADS006);
    assert!(std::error::Error::source(&error).is_some());
    assert!(error.to_string().contains(
        "construction path: failure::TopLevelService -> failure::FailingRepository -> failure::FailingDatabase"
    ));
}

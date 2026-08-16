//! Integration tests for explicit application construction.

use std::any::TypeId;
use std::sync::Arc;

use mads_core::{
    Catalog, ConstructionContext, ErasedProvider, MADS003, Mads, ProviderDescriptor,
    ProviderFuture, ProviderKind, SourceLocation,
};

struct Database;

impl Database {
    fn new() -> Self {
        Self
    }
}

struct Repository {
    database: Arc<Database>,
}

fn database_type_id() -> TypeId {
    TypeId::of::<Database>()
}

fn repository_type_id() -> TypeId {
    TypeId::of::<Repository>()
}

fn database_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async { Ok(Arc::new(Database::new()) as ErasedProvider) })
}

fn repository_constructor<'a>(context: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async move {
        let database = context.resolve::<Database>()?;
        Ok(Arc::new(Repository { database }) as ErasedProvider)
    })
}

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Provider,
        "builder::Database",
        database_type_id,
        &[],
        SourceLocation::new(file!(), line!(), column!()),
        database_constructor,
    )
}

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Repository,
        "builder::Repository",
        repository_type_id,
        &[],
        SourceLocation::new(file!(), line!(), column!()),
        repository_constructor,
    )
}

#[tokio::test]
async fn explicitly_constructs_a_provider_after_its_dependency_is_provided() {
    let mut builder = Mads::builder();
    builder
        .provide(Database::new())
        .expect("database insertion should work");
    builder
        .construct::<Repository>()
        .await
        .expect("explicit construction should work");
    let app = builder.build();

    let repository = app
        .context()
        .resolve::<Repository>()
        .expect("repository should be available after explicit construction");
    let database = app
        .context()
        .resolve::<Database>()
        .expect("provided database should remain available");
    assert!(Arc::ptr_eq(&repository.database, &database));
}

#[tokio::test]
async fn construct_does_not_recursively_create_missing_dependencies() {
    let mut builder = Mads::builder();

    let Err(error) = builder.construct::<Repository>().await else {
        panic!("repository construction should require an explicit database");
    };

    assert_eq!(error.code(), MADS003);
    assert!(Catalog::provider_for::<Database>().is_ok());
}

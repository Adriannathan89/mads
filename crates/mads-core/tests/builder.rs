//! Integration tests for explicit application construction.

use std::any::TypeId;
use std::cell::Cell;
use std::sync::{Arc, Mutex};

use mads_core::{
    ApplicationContext, Catalog, ConstructionContext, ConstructionStep, DependencyDescriptor,
    ErasedProvider, LifecycleFuture, LifecycleHook, LifecycleState, MADS003, Mads,
    ProviderDescriptor, ProviderFuture, ProviderKind, ProviderState, ProviderVisibility,
    SourceLocation,
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

thread_local! {
    static DATABASE_CONSTRUCTIONS: Cell<usize> = const { Cell::new(0) };
}

fn reset_database_constructions() {
    DATABASE_CONSTRUCTIONS.set(0);
}

fn database_constructions() -> usize {
    DATABASE_CONSTRUCTIONS.get()
}

fn database_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async {
        DATABASE_CONSTRUCTIONS.set(DATABASE_CONSTRUCTIONS.get() + 1);
        Ok(Arc::new(Database::new()) as ErasedProvider)
    })
}

fn repository_constructor<'a>(context: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async move {
        let database = context.resolve::<Database>()?;
        Ok(Arc::new(Repository { database }) as ErasedProvider)
    })
}

static REPOSITORY_DEPENDENCIES: [DependencyDescriptor; 1] = [DependencyDescriptor::new(
    "builder::Database",
    database_type_id,
)];

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Provider,
        "builder::Database",
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
        "builder::Repository",
        repository_type_id,
        &REPOSITORY_DEPENDENCIES,
        ProviderVisibility::Private,
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

#[test]
fn builder_analysis_is_repeatable_and_side_effect_free() {
    reset_database_constructions();
    let builder = Mads::builder();

    let first = builder.analyze();
    let second = builder.analyze();

    assert!(first.is_valid());
    assert!(second.is_valid());
    assert_eq!(database_constructions(), 0);
    assert_eq!(
        first
            .construction_plan()
            .unwrap()
            .steps()
            .iter()
            .map(ConstructionStep::type_name)
            .collect::<Vec<_>>(),
        second
            .construction_plan()
            .unwrap()
            .steps()
            .iter()
            .map(ConstructionStep::type_name)
            .collect::<Vec<_>>(),
    );
}

#[tokio::test]
async fn explicit_and_preconstructed_values_have_distinct_states() {
    let mut builder = Mads::builder();
    builder.provide(Database::new()).unwrap();
    builder.construct::<Repository>().await.unwrap();

    let analysis = builder.analyze();
    assert_eq!(
        analysis.graph().provider::<Database>().unwrap().state(),
        ProviderState::Provided
    );
    assert_eq!(
        analysis.graph().provider::<Repository>().unwrap().state(),
        ProviderState::Preconstructed
    );
    assert!(analysis.construction_plan().unwrap().steps().is_empty());
}

struct ApplicationHook(Arc<Mutex<Vec<&'static str>>>);

impl LifecycleHook for ApplicationHook {
    fn name(&self) -> &str {
        "application"
    }

    fn start<'a>(&'a self, _: &'a ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            self.0
                .lock()
                .expect("event lock should not be poisoned")
                .push("start");
            Ok(())
        })
    }

    fn stop<'a>(&'a self, _: &'a ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            self.0
                .lock()
                .expect("event lock should not be poisoned")
                .push("stop");
            Ok(())
        })
    }
}

#[tokio::test]
async fn built_application_owns_and_runs_registered_lifecycle_hooks() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut builder = Mads::builder();
    builder.lifecycle_hook(ApplicationHook(Arc::clone(&events)));
    let mut application = builder.build();

    assert_eq!(application.state(), LifecycleState::Created);
    application.start().await.expect("application should start");
    assert_eq!(application.state(), LifecycleState::Running);
    application
        .shutdown()
        .await
        .expect("application should stop");

    assert_eq!(application.state(), LifecycleState::Stopped);
    assert_eq!(
        *events.lock().expect("event lock should not be poisoned"),
        ["start", "stop"]
    );
}

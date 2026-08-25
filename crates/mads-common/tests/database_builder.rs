//! Explicit database bootstrap and graph integration contracts.

use std::any::TypeId;
use std::sync::{Arc, Mutex};

use mads_common::{Database, DatabaseBootstrap, DatabaseConfig, MADS100, MadsBuilderDatabaseExt};
use mads_core::{
    ApplicationContext, AutoConfigurationStatus, Catalog, LifecycleFuture, LifecycleHook, MADS001,
    MADS011, Mads, ProviderOrigin,
};

const MIGRATIONS: diesel_migrations::EmbeddedMigrations =
    diesel_migrations::embed_migrations!("tests/fixtures/compile_migrations");

#[mads_core::repository]
struct DatabaseRepository {
    database: Database,
}

impl DatabaseRepository {
    fn close_database(&self) {
        self.database.close();
    }
}

#[tokio::test]
async fn explicit_database_registration_satisfies_repository_dependency() {
    let mut builder = Mads::builder();
    builder
        .database(DatabaseBootstrap::new(
            DatabaseConfig::new("postgres://localhost/mads").unwrap(),
        ))
        .unwrap();
    let analysis = builder.analyze();
    assert_eq!(
        analysis.auto_configurations()[0].status(),
        AutoConfigurationStatus::Overridden,
    );
    assert_eq!(
        analysis.auto_configurations()[0].reason_code().as_str(),
        "user_override",
    );
    let application = builder.build().await.unwrap();

    assert_eq!(
        application.graph().provider::<Database>().unwrap().origin(),
        ProviderOrigin::Provided
    );
    let repository = application
        .context()
        .resolve::<DatabaseRepository>()
        .unwrap();
    repository.close_database();
    assert!(
        application
            .context()
            .resolve::<Database>()
            .unwrap()
            .is_closed()
    );
}

struct RecordingApplicationHook(Arc<Mutex<Vec<&'static str>>>);

impl LifecycleHook for RecordingApplicationHook {
    fn name(&self) -> &str {
        "application"
    }

    fn start<'a>(&'a self, _: &'a ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            self.0.lock().unwrap().push("start");
            Ok(())
        })
    }

    fn stop<'a>(&'a self, _: &'a ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            self.0.lock().unwrap().push("stop");
            Ok(())
        })
    }
}

#[tokio::test]
async fn explicit_database_infrastructure_failure_precedes_application_hooks() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut builder = Mads::builder();
    builder.lifecycle_hook(RecordingApplicationHook(Arc::clone(&events)));
    builder
        .database(DatabaseBootstrap::new(
            DatabaseConfig::new("postgres://localhost/mads").unwrap(),
        ))
        .unwrap();
    let mut application = builder.build().await.unwrap();
    application.context().resolve::<Database>().unwrap().close();

    let error = application.start().await.unwrap_err();

    assert_eq!(error.code(), MADS011);
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn startup_migrations_require_an_embedded_source_before_build() {
    let mut builder = Mads::builder();

    let error = database_error(
        &mut builder,
        DatabaseBootstrap::new(
            DatabaseConfig::new("postgres://localhost/mads")
                .unwrap()
                .with_migrate_on_startup(true),
        ),
    );

    assert_eq!(error.code(), MADS100);
}

#[test]
fn registering_database_twice_preserves_duplicate_provider_error() {
    let mut builder = Mads::builder();
    builder
        .database(DatabaseBootstrap::new(
            DatabaseConfig::new("postgres://localhost/mads").unwrap(),
        ))
        .unwrap();

    let error = database_error(
        &mut builder,
        DatabaseBootstrap::new(DatabaseConfig::new("postgres://localhost/mads").unwrap()),
    );

    assert_eq!(error.code(), MADS001);
}

#[test]
fn explicit_database_does_not_back_off_after_manual_provisioning() {
    let mut builder = Mads::builder();
    let config = DatabaseConfig::new("postgres://localhost/mads").unwrap();
    builder
        .provide(Database::from_config(&config).unwrap())
        .unwrap();

    let error = database_error(&mut builder, DatabaseBootstrap::new(config));

    assert_eq!(error.code(), MADS001);
}

#[test]
fn attached_migrations_are_accepted_when_startup_migrations_are_disabled() {
    let mut builder = Mads::builder();

    builder
        .database(
            DatabaseBootstrap::new(DatabaseConfig::new("postgres://localhost/mads").unwrap())
                .with_migrations(MIGRATIONS),
        )
        .unwrap();
}

#[test]
fn repository_descriptor_depends_on_public_database_type() {
    let descriptor = Catalog::provider_for::<DatabaseRepository>().unwrap();
    let dependencies = descriptor.dependencies();

    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].type_id(), TypeId::of::<Database>());
    assert_eq!(dependencies[0].type_name(), stringify!(Database));
}

fn database_error(
    builder: &mut mads_core::MadsBuilder,
    bootstrap: DatabaseBootstrap,
) -> mads_core::Error {
    match builder.database(bootstrap) {
        Ok(_) => panic!("database registration should fail"),
        Err(error) => error,
    }
}

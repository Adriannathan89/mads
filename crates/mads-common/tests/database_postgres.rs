//! Real PostgreSQL evidence for the managed database facade.

use std::sync::{Arc, Mutex};

use mads_common::{
    Database, DatabaseConfig, DatabaseErrorKind, MadsBuilderDatabaseExt,
    core::{
        ApplicationContext, AutoConfigurationStatus, ConfigBuilder, LifecycleFuture, LifecycleHook,
        Mads, MapSource, ProviderOrigin,
    },
    diesel::{self, RunQueryDsl},
    diesel_migrations::{EmbeddedMigrations, embed_migrations},
};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("tests/fixtures/migrations");

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[mads_core::repository]
struct AutoDatabaseRepository {
    database: Database,
}

impl AutoDatabaseRepository {
    fn database(&self) -> &Database {
        &self.database
    }
}

struct DatabaseShutdownHook {
    observed_open_database: Arc<Mutex<Vec<bool>>>,
}

impl LifecycleHook for DatabaseShutdownHook {
    fn name(&self) -> &str {
        "database-shutdown-observer"
    }

    fn start<'a>(&'a self, _: &'a ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async { Ok(()) })
    }

    fn stop<'a>(&'a self, context: &'a ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            let database = context.resolve::<Database>()?;
            self.observed_open_database
                .lock()
                .unwrap()
                .push(!database.is_closed());
            Ok(())
        })
    }
}

fn test_database_config() -> DatabaseConfig {
    DatabaseConfig::new(
        std::env::var("MADS_TEST_DATABASE_URL")
            .expect("MADS_TEST_DATABASE_URL is required for ignored PostgreSQL tests"),
    )
    .unwrap()
    .with_pool_size(2)
    .unwrap()
}

#[tokio::test]
#[ignore = "requires PostgreSQL through MADS_TEST_DATABASE_URL"]
async fn native_pool_queries_and_migrations_remain_available_as_an_escape_hatch() {
    let _guard = TEST_LOCK.lock().await;
    let database = Database::from_config(&test_database_config()).unwrap();
    database.check().await.unwrap();

    let applied = database.run_pending_migrations(MIGRATIONS).await.unwrap();
    assert!(applied.versions().len() <= 2);
    assert!(
        database
            .run_pending_migrations(MIGRATIONS)
            .await
            .unwrap()
            .is_empty()
    );

    database
        .run(|connection| {
            diesel::sql_query("DELETE FROM mads_common_v040_items").execute(connection)
        })
        .await
        .unwrap();
    database
        .run(|connection| {
            diesel::sql_query("INSERT INTO mads_common_v040_items (name) VALUES ('pool-proof')")
                .execute(connection)
        })
        .await
        .unwrap();

    let status = database.migration_status(MIGRATIONS).await.unwrap();
    assert!(status.pending().is_empty());
    assert_eq!(status.applied().len(), 2);
    assert_eq!(status.applied(), ["202608220101", "202608220102"]);

    let reverted = database.revert_last_migration(MIGRATIONS).await.unwrap();
    assert_eq!(reverted.versions(), ["202608220102"]);
    assert_eq!(
        database
            .migration_status(MIGRATIONS)
            .await
            .unwrap()
            .pending()
            .len(),
        1
    );
    database.run_pending_migrations(MIGRATIONS).await.unwrap();

    database.close();
    assert!(database.is_closed());
}

#[tokio::test]
#[ignore = "requires PostgreSQL through MADS_TEST_DATABASE_URL"]
async fn automatic_database_migrations_are_shared_noops_and_shutdown_after_application_hooks() {
    let _guard = TEST_LOCK.lock().await;
    let config = ConfigBuilder::new()
        .source(MapSource::new(
            "test",
            [
                (
                    "database.url",
                    std::env::var("MADS_TEST_DATABASE_URL")
                        .expect("MADS_TEST_DATABASE_URL is required for ignored PostgreSQL tests"),
                ),
                ("database.pool_size", "2".to_owned()),
                ("database.migrate", "true".to_owned()),
            ],
        ))
        .build()
        .unwrap();
    let observed_open_database = Arc::new(Mutex::new(Vec::new()));
    let mut builder = Mads::builder_with_config(config.clone());
    builder.lifecycle_hook(DatabaseShutdownHook {
        observed_open_database: Arc::clone(&observed_open_database),
    });
    builder.database_migrations(MIGRATIONS).unwrap();

    let analysis = builder.analyze();
    assert_eq!(
        analysis.auto_configurations()[0].status(),
        AutoConfigurationStatus::Active
    );
    let mut application = builder.build().await.unwrap();
    assert_eq!(
        application.graph().provider::<Database>().unwrap().origin(),
        ProviderOrigin::AutoConfiguration,
    );
    application.start().await.unwrap();
    let database = application.context().resolve::<Database>().unwrap();
    let repository = application
        .context()
        .resolve::<AutoDatabaseRepository>()
        .unwrap();
    assert!(!repository.database().is_closed());
    assert!(
        database
            .migration_status(MIGRATIONS)
            .await
            .unwrap()
            .pending()
            .is_empty()
    );
    application.shutdown().await.unwrap();
    assert_eq!(*observed_open_database.lock().unwrap(), [true]);
    assert!(database.is_closed());

    let mut second_builder = Mads::builder_with_config(config);
    second_builder.database_migrations(MIGRATIONS).unwrap();
    let mut second_application = second_builder.build().await.unwrap();
    assert_eq!(
        second_application.auto_configurations()[0].status(),
        AutoConfigurationStatus::Active
    );
    second_application.start().await.unwrap();
    let second_database = second_application.context().resolve::<Database>().unwrap();
    assert!(
        second_database
            .migration_status(MIGRATIONS)
            .await
            .unwrap()
            .pending()
            .is_empty()
    );
    second_application.shutdown().await.unwrap();
    assert!(second_database.is_closed());
}

#[tokio::test]
#[ignore = "requires PostgreSQL through MADS_TEST_DATABASE_URL"]
async fn disabled_migration_defers_schema_errors_to_the_query_boundary() {
    let _guard = TEST_LOCK.lock().await;
    let config = ConfigBuilder::new()
        .source(MapSource::new(
            "test",
            [
                (
                    "database.url",
                    std::env::var("MADS_TEST_DATABASE_URL").unwrap(),
                ),
                ("database.migrate", "false".to_owned()),
            ],
        ))
        .build()
        .unwrap();
    let mut application = Mads::builder_with_config(config).build().await.unwrap();
    application.start().await.unwrap();
    let database = application.context().resolve::<Database>().unwrap();

    let error = database
        .run(|connection| {
            diesel::sql_query("SELECT * FROM mads_v050_never_migrated").execute(connection)
        })
        .await
        .unwrap_err();
    assert_eq!(error.kind(), DatabaseErrorKind::Query);

    application.shutdown().await.unwrap();
    assert!(database.is_closed());
}

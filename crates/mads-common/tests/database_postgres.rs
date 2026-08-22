//! Real PostgreSQL evidence for the managed database facade.

use mads_common::{
    Database, DatabaseBootstrap, DatabaseConfig, MadsBuilderDatabaseExt,
    core::Mads,
    diesel::{self, RunQueryDsl},
    diesel_migrations::{EmbeddedMigrations, embed_migrations},
};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("tests/fixtures/migrations");

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
async fn pool_queries_migrations_and_lifecycle_are_real() {
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

    let config = test_database_config().with_migrate_on_startup(true);
    let mut builder = Mads::builder();
    builder
        .database(DatabaseBootstrap::new(config).with_migrations(MIGRATIONS))
        .unwrap();
    let mut application = builder.build().await.unwrap();
    application.start().await.unwrap();
    let database = application.context().resolve::<Database>().unwrap();
    assert!(!database.is_closed());
    application.shutdown().await.unwrap();
    assert!(database.is_closed());
}

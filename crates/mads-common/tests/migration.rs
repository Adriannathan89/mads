//! Offline migration API contract tests.

use mads_common::{Database, MigrationReport, MigrationStatus};

const MIGRATIONS: diesel_migrations::EmbeddedMigrations =
    diesel_migrations::embed_migrations!("tests/fixtures/compile_migrations");

#[test]
fn migration_reports_are_deterministic_value_objects() {
    let report = MigrationReport::from_versions([
        "202608220002_second".to_owned(),
        "202608220001_first".to_owned(),
        "202608220001_first".to_owned(),
    ]);
    assert_eq!(
        report.versions(),
        ["202608220001_first", "202608220002_second"]
    );
    assert!(!report.is_empty());

    let status = MigrationStatus::from_versions(
        [
            "202608220001_first".to_owned(),
            "202608220001_first".to_owned(),
        ],
        [
            "202608220003_third".to_owned(),
            "202608220002_second".to_owned(),
            "202608220002_second".to_owned(),
        ],
    );
    assert_eq!(status.applied(), ["202608220001_first"]);
    assert_eq!(
        status.pending(),
        ["202608220002_second", "202608220003_third"]
    );
}

#[test]
fn database_exposes_generic_migration_methods() {
    let _apply = Database::run_pending_migrations::<diesel_migrations::EmbeddedMigrations>;
    let _rollback = Database::revert_last_migration::<diesel_migrations::EmbeddedMigrations>;
    let _status = Database::migration_status::<diesel_migrations::EmbeddedMigrations>;
    let _file_apply = Database::run_pending_migrations::<diesel_migrations::FileBasedMigrations>;
    let _file_rollback = Database::revert_last_migration::<diesel_migrations::FileBasedMigrations>;
    let _file_status = Database::migration_status::<diesel_migrations::FileBasedMigrations>;
}

#[test]
fn embedded_migration_fixture_compiles_with_database_methods() {
    let _source = MIGRATIONS;
    let _apply = Database::run_pending_migrations::<diesel_migrations::EmbeddedMigrations>;
    let _rollback = Database::revert_last_migration::<diesel_migrations::EmbeddedMigrations>;
    let _status = Database::migration_status::<diesel_migrations::EmbeddedMigrations>;
}

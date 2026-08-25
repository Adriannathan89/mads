//! Offline behavior for the official Diesel auto-configuration.

use mads_common::{Database, DatabaseConfig, MADS101, MadsBuilderDatabaseExt};
use mads_core::{
    AutoConfigurationStatus, Config, ConfigBuilder, DotenvSource, MADS003, Mads, MapSource,
    ProviderOrigin, ProviderState,
};

const MIGRATIONS: diesel_migrations::EmbeddedMigrations =
    diesel_migrations::embed_migrations!("tests/fixtures/compile_migrations");

#[mads_core::repository]
struct AlphaRepository {
    database: Database,
}

#[mads_core::repository]
struct ZetaRepository {
    database: Database,
}

impl AlphaRepository {
    fn database(&self) -> &Database {
        &self.database
    }
}

impl ZetaRepository {
    fn database(&self) -> &Database {
        &self.database
    }
}

fn config(values: impl IntoIterator<Item = (&'static str, &'static str)>) -> Config {
    ConfigBuilder::new()
        .source(MapSource::new("test", values))
        .build()
        .unwrap()
}

#[test]
fn valid_required_database_is_active_without_creating_a_pool() {
    let builder = Mads::builder_with_config(config([(
        "database.url",
        "postgres://user:secret@localhost/mads",
    )]));
    let analysis = builder.analyze();
    let report = &analysis.auto_configurations()[0];

    assert!(analysis.is_valid());
    assert_eq!(report.identifier(), "mads.common.database.diesel");
    assert_eq!(report.status(), AutoConfigurationStatus::Active);
    assert_eq!(report.reason_code().as_str(), "conditions_matched");
    assert_eq!(report.requirements().len(), 2);
    assert!(
        report.requirements()[0]
            .provider_type_name()
            .contains("AlphaRepository")
    );
    assert!(
        report.requirements()[1]
            .provider_type_name()
            .contains("ZetaRepository")
    );
    assert_eq!(report.configuration()[0].key(), "database.url");
    let output = format!("{report:?}");
    assert!(!output.contains("secret"));
}

#[tokio::test]
async fn build_provides_database_with_explicit_auto_configured_metadata() {
    let application =
        Mads::builder_with_config(config([("database.url", "postgres://localhost/mads")]))
            .build()
            .await
            .unwrap();
    let database = application.context().resolve::<Database>().unwrap();
    let node = application.graph().provider::<Database>().unwrap();

    assert_eq!(node.origin(), ProviderOrigin::AutoConfiguration);
    assert_eq!(node.state(), ProviderState::AutoConfigured);
    assert!(!database.is_closed());
    application
        .context()
        .resolve::<AlphaRepository>()
        .unwrap()
        .database()
        .close();
    application
        .context()
        .resolve::<ZetaRepository>()
        .unwrap()
        .database()
        .close();
}

#[test]
fn missing_url_is_mads101_without_redundant_mads003() {
    let analysis = Mads::builder().analyze();
    assert!(!analysis.is_valid());
    assert_eq!(
        analysis.auto_configurations()[0].status(),
        AutoConfigurationStatus::Failed
    );
    assert_eq!(
        analysis.auto_configurations()[0].reason_code().as_str(),
        "missing_configuration"
    );
    assert_eq!(analysis.diagnostics()[0].code(), MADS101);
    assert!(
        analysis
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code() != MADS003)
    );
    let rendered = analysis.diagnostics()[0].to_string();
    assert!(rendered.contains("database.url"));
    assert!(rendered.contains("AlphaRepository"));
    assert!(rendered.contains("ZetaRepository"));
    assert!(rendered.contains("database_auto_configuration.rs"));
    assert!(rendered.contains("configure `database.url`"));
    assert!(rendered.contains("explicit or custom `Database`"));
}

#[test]
fn invalid_configuration_is_mads101_and_redacted() {
    let database_url = "postgres://user:invalid-secret@localhost/mads";
    let analysis = Mads::builder_with_config(config([
        ("database.url", database_url),
        ("database.pool_size", "0"),
    ]))
    .analyze();
    assert_eq!(analysis.diagnostics()[0].code(), MADS101);
    assert_eq!(
        analysis.auto_configurations()[0].reason_code().as_str(),
        "invalid_configuration"
    );
    assert!(!format!("{:?}", analysis.auto_configurations()).contains("invalid-secret"));
    let rendered = analysis.diagnostics()[0].to_string();
    assert!(rendered.contains("database.url"));
    assert!(rendered.contains("AlphaRepository"));
    assert!(rendered.contains("ZetaRepository"));
    assert!(rendered.contains("database_auto_configuration.rs"));
    assert!(rendered.contains("configure `database.url`"));
    assert!(rendered.contains("explicit or custom `Database`"));
}

#[test]
fn non_recursive_unresolved_url_is_mads101() {
    let directory = tempfile::tempdir().unwrap();
    let dotenv = directory.path().join("nested.env");
    std::fs::write(&dotenv, "DATABASE_URL='${STILL_UNRESOLVED}'\n").unwrap();
    let config = ConfigBuilder::new()
        .dotenv(DotenvSource::required(dotenv))
        .source(MapSource::new(
            "mads.toml",
            [("database.url", "${DATABASE_URL}")],
        ))
        .build()
        .unwrap();
    let analysis = Mads::builder_with_config(config).analyze();

    assert_eq!(analysis.diagnostics()[0].code(), MADS101);
    assert_eq!(
        analysis.auto_configurations()[0].reason_code().as_str(),
        "invalid_configuration",
    );
    assert_eq!(
        analysis.auto_configurations()[0].configuration()[0].source(),
        Some("mads.toml"),
    );
    assert!(!format!("{:?}", analysis.auto_configurations()).contains("STILL_UNRESOLVED"));
}

#[test]
fn enabled_migration_requires_exactly_one_registered_source() {
    let config = config([
        ("database.url", "postgres://localhost/mads"),
        ("database.migrate", "true"),
    ]);
    let mut missing = Mads::builder_with_config(config.clone());
    assert_eq!(missing.analyze().diagnostics()[0].code(), MADS101);
    assert_eq!(
        missing.analyze().auto_configurations()[0]
            .reason_code()
            .as_str(),
        "missing_migration_source",
    );

    missing.database_migrations(MIGRATIONS).unwrap();
    assert!(missing.analyze().is_valid());
    let Err(duplicate) = missing.database_migrations(MIGRATIONS) else {
        panic!("the duplicate migration source must be rejected");
    };
    assert_eq!(duplicate.code(), MADS101);
}

#[test]
fn disabled_migration_accepts_but_does_not_require_a_source() {
    let mut builder = Mads::builder_with_config(config([
        ("database.url", "postgres://localhost/mads"),
        ("database.migrate", "false"),
    ]));
    builder.database_migrations(MIGRATIONS).unwrap();
    let analysis = builder.analyze();
    assert!(analysis.is_valid());
    assert_eq!(
        analysis.auto_configurations()[0].explanation(),
        "Database is required and configured with startup migrations disabled",
    );
}

#[tokio::test]
async fn provided_database_overrides_invalid_default_before_parsing() {
    let config = config([("database.url", ""), ("database.pool_size", "0")]);
    let provided =
        Database::from_config(&DatabaseConfig::new("postgres://localhost/custom").unwrap())
            .unwrap();
    let mut builder = Mads::builder_with_config(config);
    builder.database_migrations(MIGRATIONS).unwrap();
    builder.provide(provided).unwrap();
    let application = builder.build().await.unwrap();
    assert_eq!(
        application.auto_configurations()[0].status(),
        AutoConfigurationStatus::Overridden,
    );
    application.context().resolve::<Database>().unwrap().close();
}

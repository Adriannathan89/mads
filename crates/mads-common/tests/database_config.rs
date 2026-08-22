//! Public database configuration contracts.

use mads_common::{DatabaseConfig, DatabaseErrorKind};
use mads_core::{ConfigBuilder, DotenvSource, MapSource};

#[test]
fn database_config_parses_defaults_and_explicit_values() {
    let defaults = ConfigBuilder::new()
        .source(MapSource::new(
            "test",
            [("database.url", "postgres://user:secret@localhost/app")],
        ))
        .build()
        .unwrap();
    let defaults = DatabaseConfig::from_config(&defaults).unwrap();
    assert_eq!(defaults.pool_size(), 10);
    assert!(!defaults.migrate_on_startup());

    let explicit = ConfigBuilder::new()
        .source(MapSource::new(
            "test",
            [
                ("database.url", "postgres://localhost/app"),
                ("database.pool_size", "4"),
                ("database.migrate", "true"),
            ],
        ))
        .build()
        .unwrap();
    let explicit = DatabaseConfig::from_config(&explicit).unwrap();
    assert_eq!(explicit.pool_size(), 4);
    assert!(explicit.migrate_on_startup());
}

#[test]
fn configuration_failures_are_classified_and_redacted() {
    let config = ConfigBuilder::new()
        .source(MapSource::new(
            "test",
            [
                ("database.url", "postgres://user:top-secret@localhost/app"),
                ("database.pool_size", "0"),
            ],
        ))
        .build()
        .unwrap();
    let error = DatabaseConfig::from_config(&config).unwrap_err();
    assert_eq!(error.kind(), DatabaseErrorKind::Configuration);
    assert!(!format!("{error:?}").contains("top-secret"));
    assert!(!error.to_string().contains("top-secret"));
}

#[test]
fn invalid_url_values_are_configuration_errors_without_value_echoes() {
    for value in [None, Some("")] {
        let config = ConfigBuilder::new()
            .source(MapSource::new(
                "test",
                value.map(|value| ("database.url", value)),
            ))
            .build()
            .unwrap();

        let error = DatabaseConfig::from_config(&config).unwrap_err();
        assert_eq!(error.kind(), DatabaseErrorKind::Configuration);
        assert!(error.to_string().contains("database.url"));
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            assert!(!error.to_string().contains(value));
        }
    }
}

#[test]
fn unresolved_exact_placeholder_is_a_redacted_configuration_error() {
    let path = temporary_dotenv_path();
    std::fs::write(&path, "DATABASE_URL=${NAME}\n").unwrap();
    let config = ConfigBuilder::new()
        .dotenv(DotenvSource::required(&path))
        .source(MapSource::new(
            "test",
            [("database.url", "${DATABASE_URL}")],
        ))
        .build()
        .unwrap();
    std::fs::remove_file(path).unwrap();

    let error = DatabaseConfig::from_config(&config).unwrap_err();
    assert_eq!(error.kind(), DatabaseErrorKind::Configuration);
    assert!(error.to_string().contains("database.url"));
    assert!(!error.to_string().contains("${NAME}"));
}

#[test]
fn invalid_pool_sizes_are_configuration_errors_without_value_echoes() {
    for value in ["0", "-1", "many", "999999999999999999999999999999999999"] {
        let config = config_with([("database.pool_size", value)]);
        let error = DatabaseConfig::from_config(&config).unwrap_err();
        assert_eq!(error.kind(), DatabaseErrorKind::Configuration);
        assert!(error.to_string().contains("database.pool_size"));
        assert!(!error.to_string().contains(value));
    }
}

#[test]
fn invalid_migrate_values_are_configuration_errors_without_value_echoes() {
    for value in ["TRUE", "yes", "0"] {
        let config = config_with([("database.migrate", value)]);
        let error = DatabaseConfig::from_config(&config).unwrap_err();
        assert_eq!(error.kind(), DatabaseErrorKind::Configuration);
        assert!(error.to_string().contains("database.migrate"));
        assert!(!error.to_string().contains(value));
    }
}

#[test]
fn embedded_placeholder_is_an_ordinary_url_literal() {
    let config = config_with([("database.url", "prefix-${NAME}")]);
    let config = DatabaseConfig::from_config(&config).unwrap();
    assert_eq!(config.url(), "prefix-${NAME}");
}

#[test]
fn database_config_debug_redacts_the_url() {
    let config = DatabaseConfig::new("postgres://user:top-secret@localhost/app").unwrap();
    let debug = format!("{config:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("top-secret"));
}

fn config_with<const N: usize>(values: [(&str, &str); N]) -> mads_core::Config {
    let mut values = values.to_vec();
    if !values.iter().any(|(key, _)| *key == "database.url") {
        values.push(("database.url", "postgres://localhost/app"));
    }
    ConfigBuilder::new()
        .source(MapSource::new("test", values))
        .build()
        .unwrap()
}

fn temporary_dotenv_path() -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "mads-common-database-config-{}-{nonce}.env",
        std::process::id()
    ))
}

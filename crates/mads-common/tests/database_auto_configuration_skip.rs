//! Requirement-free behavior for the official Diesel auto-configuration.

#![cfg(feature = "database")]

use mads_common::Database;
use mads_core::{AutoConfigurationStatus, ConfigBuilder, Mads, MapSource};

#[test]
fn unused_invalid_database_configuration_is_skipped_without_validation() {
    let _ = Database::is_closed as fn(&Database) -> bool;
    let config = ConfigBuilder::new()
        .source(MapSource::new(
            "test",
            [("database.url", ""), ("database.pool_size", "0")],
        ))
        .build()
        .unwrap();
    let analysis = Mads::builder_with_config(config).analyze();
    assert!(analysis.is_valid());
    assert_eq!(
        analysis.auto_configurations()[0].status(),
        AutoConfigurationStatus::Skipped
    );
    assert_eq!(
        analysis.auto_configurations()[0].reason_code().as_str(),
        "requirement_absent"
    );
    assert!(analysis.auto_configurations()[0].configuration().is_empty());
}

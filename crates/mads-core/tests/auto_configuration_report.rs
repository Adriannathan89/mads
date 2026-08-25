//! Public auto-configuration inspection records.

use mads_core::{
    AutoConfigurationConfigEvidence, AutoConfigurationReasonCode, AutoConfigurationReport,
    AutoConfigurationRequirement, AutoConfigurationStatus, ConfigBuilder, MADS007, MapSource,
    SourceLocation,
};

#[test]
fn report_accessors_preserve_stable_redacted_evidence() {
    let requirement = AutoConfigurationRequirement::new(
        "example::Repository",
        Some(SourceLocation::new("src/repository.rs", 7, 9)),
    );
    let config = ConfigBuilder::new()
        .source(MapSource::new("mads.toml", [("database.url", "ignored")]))
        .build()
        .unwrap();
    let evidence = AutoConfigurationConfigEvidence::new("database.url", &config);
    let report = AutoConfigurationReport::new(
        "mads.common.database.diesel",
        "mads_common::Database",
        AutoConfigurationStatus::Active,
        AutoConfigurationReasonCode::new("conditions_matched"),
        "Database is required and configured",
        vec![requirement],
        vec![evidence],
    );

    assert_eq!(report.identifier(), "mads.common.database.diesel");
    assert_eq!(report.output_type_name(), "mads_common::Database");
    assert_eq!(report.status(), AutoConfigurationStatus::Active);
    assert_eq!(report.reason_code().as_str(), "conditions_matched");
    assert_eq!(report.explanation(), "Database is required and configured");
    assert_eq!(
        report.requirements()[0].provider_type_name(),
        "example::Repository"
    );
    assert_eq!(
        report.requirements()[0].location(),
        Some(SourceLocation::new("src/repository.rs", 7, 9)),
    );
    assert_eq!(report.configuration()[0].key(), "database.url");
    assert_eq!(report.configuration()[0].source(), Some("mads.toml"));
    assert_eq!(MADS007.as_str(), "MADS007");
}

#[test]
fn every_status_has_the_expected_debug_name() {
    assert_eq!(format!("{:?}", AutoConfigurationStatus::Active), "Active");
    assert_eq!(format!("{:?}", AutoConfigurationStatus::Skipped), "Skipped");
    assert_eq!(
        format!("{:?}", AutoConfigurationStatus::Overridden),
        "Overridden"
    );
    assert_eq!(format!("{:?}", AutoConfigurationStatus::Failed), "Failed");
}

#[test]
fn report_construction_redacts_secret_like_text() {
    let config = ConfigBuilder::new()
        .source(MapSource::new(
            "postgres://user:dotenv-secret@localhost/mads",
            [("database.url", "ignored")],
        ))
        .build()
        .unwrap();
    let evidence = AutoConfigurationConfigEvidence::new("database.url", &config);
    let report = AutoConfigurationReport::new(
        "mads.common.database.diesel",
        "mads_common::Database",
        AutoConfigurationStatus::Failed,
        AutoConfigurationReasonCode::new("invalid_configuration"),
        "postgres://user:process-secret@localhost/mads",
        Vec::new(),
        vec![evidence],
    );

    assert_eq!(report.explanation(), "<redacted>");
    assert_eq!(report.configuration()[0].source(), Some("<redacted>"));
    let debug = format!("{report:?}");
    assert!(!debug.contains("dotenv-secret"));
    assert!(!debug.contains("process-secret"));
}

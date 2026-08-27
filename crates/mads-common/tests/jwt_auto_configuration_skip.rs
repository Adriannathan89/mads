//! Requirement-free behavior for the official JWT auto-configuration.

#![cfg(feature = "jwt")]

use mads_common::JwtService;
use mads_core::{AutoConfigurationStatus, ConfigBuilder, Mads, MapSource};

#[test]
fn unused_invalid_passport_configuration_is_skipped_without_validation() {
    let _ = JwtService::from_config as fn(&mads_core::Config) -> _;
    let config = ConfigBuilder::new()
        .source(MapSource::new(
            "mads.toml",
            [
                ("passport.secret", ""),
                ("passport.clock_skew_seconds", "not-a-number"),
            ],
        ))
        .build()
        .unwrap();
    let analysis = Mads::builder_with_config(config).analyze();
    let report = analysis
        .auto_configurations()
        .iter()
        .find(|report| report.identifier() == "mads.common.passport.jwt")
        .expect("the JWT auto-configuration descriptor must be registered");

    assert!(analysis.is_valid());
    assert_eq!(report.status(), AutoConfigurationStatus::Skipped);
    assert_eq!(report.reason_code().as_str(), "requirement_absent");
    assert!(report.configuration().is_empty());
}

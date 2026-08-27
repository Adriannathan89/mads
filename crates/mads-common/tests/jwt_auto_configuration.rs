//! Activation and provisioning behavior for the official JWT default.

#![cfg(feature = "jwt")]

use std::time::Duration;

use mads_common::{JwtService, JwtSignOptions, JwtValidation, MADS121};
use mads_core::{
    AutoConfigurationStatus, Config, ConfigBuilder, MADS003, Mads, MapSource, ProviderOrigin,
};

#[mads_core::service]
struct TokenIssuer {
    jwt: JwtService,
}

fn config(values: impl IntoIterator<Item = (&'static str, &'static str)>) -> Config {
    ConfigBuilder::new()
        .source(MapSource::new("mads.toml", values))
        .build()
        .unwrap()
}

fn jwt_report(analysis: &mads_core::GraphAnalysis) -> &mads_core::AutoConfigurationReport {
    analysis
        .auto_configurations()
        .iter()
        .find(|report| report.identifier() == "mads.common.passport.jwt")
        .expect("the JWT auto-configuration descriptor must be registered")
}

#[test]
fn direct_requirement_activates_the_jwt_default() {
    let analysis = Mads::builder_with_config(config([(
        "passport.secret",
        "01234567890123456789012345678901",
    )]))
    .analyze();
    let report = jwt_report(&analysis);

    assert!(analysis.is_valid());
    assert_eq!(report.status(), AutoConfigurationStatus::Active);
    assert_eq!(report.reason_code().as_str(), "conditions_matched");
    assert!(
        report.requirements()[0]
            .provider_type_name()
            .contains("TokenIssuer")
    );
    assert_eq!(report.configuration()[0].key(), "passport.secret");
    assert_eq!(report.configuration()[0].source(), Some("mads.toml"));
    assert!(!format!("{report:?}").contains("01234567890123456789012345678901"));
}

#[test]
fn missing_required_configuration_is_mads121_without_mads003() {
    let analysis = Mads::builder().analyze();
    let report = jwt_report(&analysis);

    assert!(!analysis.is_valid());
    assert_eq!(report.status(), AutoConfigurationStatus::Failed);
    assert_eq!(report.reason_code().as_str(), "missing_configuration");
    assert_eq!(analysis.diagnostics()[0].code(), MADS121);
    assert!(
        analysis
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code() != MADS003)
    );
    let rendered = analysis.diagnostics()[0].to_string();
    assert!(rendered.contains("passport.secret"));
    assert!(rendered.contains("TokenIssuer"));
    assert!(rendered.contains("explicit `JwtService`"));
}

#[test]
fn invalid_required_configuration_is_mads121_and_redacted() {
    const SENTINEL: &str = "jwt-secret-never-display";
    let analysis = Mads::builder_with_config(config([("passport.secret", SENTINEL)])).analyze();
    let report = jwt_report(&analysis);

    assert!(!analysis.is_valid());
    assert_eq!(report.status(), AutoConfigurationStatus::Failed);
    assert_eq!(report.reason_code().as_str(), "invalid_configuration");
    assert_eq!(analysis.diagnostics()[0].code(), MADS121);
    assert!(
        analysis
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code() != MADS003)
    );
    assert!(!format!("{:?}", analysis.auto_configurations()).contains(SENTINEL));
    assert!(!analysis.diagnostics()[0].to_string().contains(SENTINEL));
}

#[tokio::test]
async fn build_injects_the_cloneable_auto_configured_service_into_its_consumer() {
    let application = Mads::builder_with_config(config([(
        "passport.secret",
        "01234567890123456789012345678901",
    )]))
    .build()
    .await
    .unwrap();
    let issuer = application.context().resolve::<TokenIssuer>().unwrap();
    let service = application.context().resolve::<JwtService>().unwrap();
    let token = issuer
        .jwt
        .clone()
        .sign(
            serde_json::json!({ "user_id": 7 }),
            JwtSignOptions::access(Duration::from_secs(60)),
        )
        .unwrap();

    assert!(
        service
            .verify::<serde_json::Value>(&token, JwtValidation::access())
            .is_ok()
    );
    assert_eq!(
        application
            .graph()
            .provider::<JwtService>()
            .unwrap()
            .origin(),
        ProviderOrigin::AutoConfiguration,
    );
}

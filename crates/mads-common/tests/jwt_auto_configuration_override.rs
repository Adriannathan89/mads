//! Application-provided JWT services take precedence over the official default.

#![cfg(feature = "jwt")]

use mads_common::{JwtService, PassportConfig};
use mads_core::{AutoConfigurationStatus, Config, ConfigBuilder, Mads, MapSource, ProviderOrigin};

#[mads_core::service]
struct OverrideConsumer {
    jwt: JwtService,
}

fn config(values: impl IntoIterator<Item = (&'static str, &'static str)>) -> Config {
    ConfigBuilder::new()
        .source(MapSource::new("mads.toml", values))
        .build()
        .unwrap()
}

fn explicit_jwt_service() -> JwtService {
    let config = config([("passport.secret", "01234567890123456789012345678901")]);
    JwtService::from_passport_config(PassportConfig::from_config(&config).unwrap()).unwrap()
}

#[tokio::test]
async fn provided_service_overrides_invalid_defaults_before_passport_is_parsed() {
    let mut builder = Mads::builder_with_config(config([
        ("passport.secret", ""),
        ("passport.clock_skew_seconds", "not-a-number"),
    ]));
    builder.provide(explicit_jwt_service()).unwrap();
    let analysis = builder.analyze();
    let report = analysis
        .auto_configurations()
        .iter()
        .find(|report| report.identifier() == "mads.common.passport.jwt")
        .expect("the JWT auto-configuration descriptor must be registered");

    assert!(analysis.is_valid());
    assert_eq!(report.status(), AutoConfigurationStatus::Overridden);
    assert_eq!(report.reason_code().as_str(), "user_override");
    assert!(report.configuration().is_empty());

    let application = builder.build().await.unwrap();
    assert_eq!(
        application
            .graph()
            .provider::<JwtService>()
            .unwrap()
            .origin(),
        ProviderOrigin::Provided,
    );
    let _cloneable_service: JwtService = application
        .context()
        .resolve::<OverrideConsumer>()
        .unwrap()
        .jwt
        .clone();
}

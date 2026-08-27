//! Built-in `jwt` guards activate the official JWT default.

#![cfg(all(feature = "http", feature = "jwt"))]

use mads_common::{
    ClaimsPrincipal, GuardCatalog, JwtService, JwtTokenKind, PassportPrincipal,
    PassportStrategyCatalog,
    core::{AutoConfigurationStatus, Config, ConfigBuilder, Mads, MapSource},
};

#[derive(serde::Deserialize)]
struct UserClaims;

impl PassportPrincipal for UserClaims {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

#[mads_common::routes(prefix = "/users")]
#[mads_common::guard(strategy = "jwt", principal = ClaimsPrincipal<UserClaims>)]
#[allow(dead_code)]
trait UserRoutes {
    #[mads_common::get("/profile")]
    async fn profile(&self);
}

fn config(values: impl IntoIterator<Item = (&'static str, &'static str)>) -> Config {
    ConfigBuilder::new()
        .source(MapSource::new("mads.toml", values))
        .build()
        .unwrap()
}

#[test]
fn claims_principal_selects_the_built_in_jwt_adapter() {
    let guards = GuardCatalog::guards();
    let preflight = PassportStrategyCatalog::preflight(&guards).unwrap();
    let binding = preflight.bindings().first().unwrap();

    assert_eq!(binding.strategy(), "jwt");
    assert_eq!(binding.token_kind(), JwtTokenKind::Access);
    assert!(binding.is_builtin());
}

#[test]
fn guarded_route_activates_jwt_without_a_direct_provider_requirement() {
    let analysis = Mads::builder_with_config(config([(
        "passport.secret",
        "01234567890123456789012345678901",
    )]))
    .analyze();
    let report = analysis
        .auto_configurations()
        .iter()
        .find(|report| report.identifier() == "mads.common.passport.jwt")
        .unwrap();

    assert!(analysis.is_valid());
    assert_eq!(report.status(), AutoConfigurationStatus::Active);
    assert_eq!(report.reason_code().as_str(), "conditions_matched");
    assert_eq!(report.requirements().len(), 1);
    assert_eq!(
        report.requirements()[0].provider_type_name(),
        "UserRoutes::profile"
    );
    assert!(report.requirements()[0].location().is_some());
}

#[test]
fn explicit_jwt_service_still_overrides_guard_driven_configuration_before_parsing() {
    let valid_config = config([("passport.secret", "01234567890123456789012345678901")]);
    let service = JwtService::from_config(&valid_config).unwrap();
    let mut builder = Mads::builder_with_config(config([
        ("passport.secret", ""),
        ("passport.clock_skew_seconds", "not-a-number"),
    ]));
    builder.provide(service).unwrap();

    let analysis = builder.analyze();
    let report = analysis
        .auto_configurations()
        .iter()
        .find(|report| report.identifier() == "mads.common.passport.jwt")
        .unwrap();

    assert!(analysis.is_valid());
    assert_eq!(report.status(), AutoConfigurationStatus::Overridden);
    assert_eq!(report.reason_code().as_str(), "user_override");
    assert!(report.configuration().is_empty());
    assert_eq!(
        report.requirements()[0].provider_type_name(),
        "UserRoutes::profile"
    );
}

//! Official Passport JWT auto-configuration conditions and provisioning.

use std::any::TypeId;

use mads_core::__private::{
    AutoConfigurationApplyContext, AutoConfigurationContext, AutoConfigurationContribution,
    AutoConfigurationDescriptor, AutoConfigurationEvaluation,
};
use mads_core::{
    AutoConfigurationConfigEvidence, AutoConfigurationReasonCode, AutoConfigurationRequirement,
    Diagnostic, Error, Result, SourceLocation,
};

use super::{JwtService, MADS121, PassportConfig};

/// Stable identifier for the official Passport JWT auto-configuration.
pub(super) const JWT_AUTO_CONFIGURATION_ID: &str = "mads.common.passport.jwt";

const JWT_SERVICE_TYPE_NAME: &str = "mads_common::jwt::service::JwtService";
const PASSPORT_CONFIGURATION_KEYS: [&str; 7] = [
    "passport.secret",
    "passport.algorithms",
    "passport.active_key",
    "passport.issuer",
    "passport.audiences",
    "passport.clock_skew_seconds",
    "passport.max_token_bytes",
];

fn jwt_service_type_id() -> TypeId {
    TypeId::of::<JwtService>()
}

fn evaluate(context: &AutoConfigurationContext<'_>) -> AutoConfigurationEvaluation {
    if context.has_provider::<JwtService>() {
        return AutoConfigurationEvaluation::overridden(
            AutoConfigurationReasonCode::new("user_override"),
            "an application provider overrides the Passport JWT default",
            context.requirements::<JwtService>(),
            Vec::new(),
        );
    }

    let requirements = context.requirements::<JwtService>();
    if requirements.is_empty() {
        return AutoConfigurationEvaluation::skipped(
            AutoConfigurationReasonCode::new("requirement_absent"),
            "no provider requires the Passport JWT default",
            requirements,
            Vec::new(),
        );
    }

    let configuration = configuration_evidence(context);
    match PassportConfig::from_config(context.config()) {
        Ok(_) => AutoConfigurationEvaluation::active(
            AutoConfigurationReasonCode::new("conditions_matched"),
            "Passport JWT is required and configured",
            requirements,
            configuration,
        ),
        Err(_) => {
            let missing = signing_configuration_is_missing(context.config());
            let reason = if missing {
                "missing_configuration"
            } else {
                "invalid_configuration"
            };
            AutoConfigurationEvaluation::failed(
                AutoConfigurationReasonCode::new(reason),
                "Passport JWT auto-configuration could not read its configuration",
                requirements.clone(),
                configuration,
                configuration_error(missing, &requirements),
            )
        }
    }
}

fn signing_configuration_is_missing(config: &mads_core::Config) -> bool {
    config.get("passport.secret").is_none()
        && config.get_string_array("passport.secret").is_none()
        && config.get("passport.active_key").is_none()
        && config.get_string_array("passport.active_key").is_none()
        && !config
            .iter()
            .any(|(key, _)| key.starts_with("passport.keys."))
        && !config
            .iter_string_arrays()
            .any(|(key, _)| key.starts_with("passport.keys."))
}

fn configuration_error(missing: bool, requirements: &[AutoConfigurationRequirement]) -> Error {
    let (subject, message) = if missing {
        ("passport.secret", "passport.secret is missing")
    } else {
        ("passport", "Passport JWT configuration is invalid")
    };
    let requiring_providers = requirements
        .iter()
        .map(|requirement| match requirement.location() {
            Some(location) => format!(
                "{} at {}:{}:{}",
                requirement.provider_type_name(),
                location.file,
                location.line,
                location.column
            ),
            None => requirement.provider_type_name().to_owned(),
        })
        .collect::<Vec<_>>()
        .join(", ");

    Error::new(
        Diagnostic::new(
            MADS121,
            "JWT auto-configuration is invalid",
            format!("{message}; required by {requiring_providers}"),
        )
        .with_subject(subject)
        .with_suggestion("configure `passport.secret` or a valid named Passport key ring")
        .with_suggestion("provide an explicit `JwtService` to override the default"),
    )
}

fn apply(context: &AutoConfigurationApplyContext<'_>) -> Result<AutoConfigurationContribution> {
    let config = PassportConfig::from_config(context.config()).map_err(provisioning_error)?;
    let service = JwtService::from_passport_config(config).map_err(provisioning_error)?;
    Ok(AutoConfigurationContribution::new(service))
}

fn provisioning_error(_: super::JwtError) -> Error {
    Error::new(
        Diagnostic::new(
            MADS121,
            "JWT auto-configuration failed",
            "Passport JWT key provisioning failed",
        )
        .with_subject("passport")
        .with_suggestion("verify the configured Passport key material is available")
        .with_suggestion("provide an explicit `JwtService` to override the default"),
    )
}

fn configuration_evidence(
    context: &AutoConfigurationContext<'_>,
) -> Vec<AutoConfigurationConfigEvidence> {
    PASSPORT_CONFIGURATION_KEYS
        .into_iter()
        .filter_map(|key| {
            let source = context
                .config()
                .source_of(key)
                .or_else(|| context.config().source_of_string_array(key));
            source.map(|source| AutoConfigurationConfigEvidence::new(key, Some(source)))
        })
        .collect()
}

mads_core::__private::inventory::submit! {
    AutoConfigurationDescriptor::new(
        JWT_AUTO_CONFIGURATION_ID,
        JWT_SERVICE_TYPE_NAME,
        jwt_service_type_id,
        SourceLocation::new(file!(), line!(), column!()),
        evaluate,
        apply,
    )
}

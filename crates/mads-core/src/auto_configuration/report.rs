use std::path::Path;

use crate::SourceLocation;

const REDACTED: &str = "<redacted>";

fn safe_source_label(value: &str) -> &'static str {
    match value {
        "defaults" => "defaults",
        "environment" => "environment",
        "mads.toml" => "mads.toml",
        "test" => "test",
        source
            if Path::new(source)
                .file_name()
                .is_some_and(|name| name == "mads.toml") =>
        {
            "mads.toml"
        }
        _ => REDACTED,
    }
}

fn safe_explanation(reason_code: AutoConfigurationReasonCode, explanation: &str) -> &'static str {
    if explanation == "Database is required and configured" {
        return "Database is required and configured";
    }
    if explanation == "Database is required and configured with startup migrations disabled" {
        return "Database is required and configured with startup migrations disabled";
    }

    match reason_code.as_str() {
        "conditions_matched" => "Auto-configuration conditions matched",
        "requirement_absent" => "No provider requires this auto-configuration",
        "user_override" => "An application provider overrides this auto-configuration",
        "missing_configuration" => "Required configuration is missing",
        "invalid_configuration" => "Required configuration is invalid",
        "missing_migration_source" => "A required migration source is missing",
        "provisioning_failed" => "Auto-configuration provisioning failed",
        "duplicate_identifier" => "An auto-configuration identifier is duplicated",
        "conflicting_default" => "Auto-configuration defaults conflict",
        _ => REDACTED,
    }
}

/// The outcome of evaluating an auto-configuration descriptor.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AutoConfigurationStatus {
    /// The descriptor produced its configured provider.
    Active,
    /// The descriptor did not apply to the application.
    Skipped,
    /// An explicit provider took precedence over the descriptor.
    Overridden,
    /// The descriptor matched but could not produce its provider.
    Failed,
}

/// A stable, machine-readable reason for an auto-configuration outcome.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AutoConfigurationReasonCode(&'static str);

impl AutoConfigurationReasonCode {
    /// Creates a reason code from a stable string.
    #[doc(hidden)]
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the stable string representation of this reason code.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// A provider requirement that contributed to an auto-configuration decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoConfigurationRequirement {
    provider_type_name: &'static str,
    location: Option<SourceLocation>,
}

impl AutoConfigurationRequirement {
    /// Creates redacted provider requirement evidence for an integration.
    #[doc(hidden)]
    pub const fn new(provider_type_name: &'static str, location: Option<SourceLocation>) -> Self {
        Self {
            provider_type_name,
            location,
        }
    }

    /// Returns the provider type name that established the requirement.
    pub const fn provider_type_name(&self) -> &'static str {
        self.provider_type_name
    }

    /// Returns the source location that established the requirement, when known.
    pub const fn location(&self) -> Option<SourceLocation> {
        self.location
    }
}

/// Redacted evidence that a configuration key was present in a named source.
///
/// Configuration values are never stored in evidence. The optional source is
/// a fixed, recognized human-readable source label. Unknown labels are
/// redacted before storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoConfigurationConfigEvidence {
    key: &'static str,
    source: Option<String>,
}

impl AutoConfigurationConfigEvidence {
    /// Creates redacted configuration evidence from a configuration source label.
    #[doc(hidden)]
    pub fn new(key: &'static str, source: Option<&str>) -> Self {
        Self {
            key,
            source: source.map(|source| safe_source_label(source).to_owned()),
        }
    }

    /// Returns the configuration key whose presence contributed to the decision.
    pub const fn key(&self) -> &'static str {
        self.key
    }

    /// Returns the configuration source label, when one is available.
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }
}

/// A redacted report describing one auto-configuration decision.
///
/// Reports contain only stable identifiers, type and key names, source labels,
/// and explanatory text. They never store resolved configuration values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoConfigurationReport {
    identifier: &'static str,
    output_type_name: &'static str,
    status: AutoConfigurationStatus,
    reason_code: AutoConfigurationReasonCode,
    explanation: String,
    requirements: Vec<AutoConfigurationRequirement>,
    configuration: Vec<AutoConfigurationConfigEvidence>,
}

impl AutoConfigurationReport {
    /// Creates an auto-configuration report and redacts sensitive explanation text.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identifier: &'static str,
        output_type_name: &'static str,
        status: AutoConfigurationStatus,
        reason_code: AutoConfigurationReasonCode,
        explanation: impl AsRef<str>,
        requirements: Vec<AutoConfigurationRequirement>,
        configuration: Vec<AutoConfigurationConfigEvidence>,
    ) -> Self {
        Self {
            identifier,
            output_type_name,
            status,
            reason_code,
            explanation: safe_explanation(reason_code, explanation.as_ref()).to_owned(),
            requirements,
            configuration,
        }
    }

    /// Returns the stable descriptor identifier.
    pub const fn identifier(&self) -> &'static str {
        self.identifier
    }

    /// Returns the type name of the provider the descriptor can produce.
    pub const fn output_type_name(&self) -> &'static str {
        self.output_type_name
    }

    /// Returns the outcome of evaluating the descriptor.
    pub const fn status(&self) -> AutoConfigurationStatus {
        self.status
    }

    /// Returns the stable reason code for the outcome.
    pub const fn reason_code(&self) -> AutoConfigurationReasonCode {
        self.reason_code
    }

    /// Returns the human-readable explanation of the outcome.
    pub fn explanation(&self) -> &str {
        &self.explanation
    }

    /// Returns the provider requirements that contributed to the decision.
    pub fn requirements(&self) -> &[AutoConfigurationRequirement] {
        &self.requirements
    }

    /// Returns redacted configuration evidence that contributed to the decision.
    pub fn configuration(&self) -> &[AutoConfigurationConfigEvidence] {
        &self.configuration
    }
}

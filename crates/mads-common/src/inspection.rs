//! Private, owned application inspection reports.
#![allow(dead_code)] // Task 3 invokes the report assembler from the standard-run interception.

use std::path::Path;

use mads_core::{
    AutoConfigurationStatus, Config, Diagnostic, GraphInspectionSnapshot, Mads, Module,
    ModuleGraph, ProviderOrigin, ProviderState, ProviderVisibility,
};
use serde::{Deserialize, Serialize};

use crate::cors::{CORS_AUTO_CONFIGURATION_ID, enable_automatic_cors};
use crate::http_scope::HttpApplicationScope;
use crate::route::validate_scoped_descriptors;
use crate::server_config::{
    SERVER_AUTO_CONFIGURATION_ID, enable_automatic_server, load_standard_config_from,
};

/// Version of the private application-inspection transport.
#[doc(hidden)]
pub const INSPECTION_PROTOCOL_VERSION: u32 = 1;
/// Environment key carrying the requested private transport version.
#[doc(hidden)]
pub const INSPECTION_VERSION_ENV: &str = "MADS_INTERNAL_INSPECTION_VERSION";
/// Environment key carrying the requested inspection kind.
#[doc(hidden)]
pub const INSPECTION_KIND_ENV: &str = "MADS_INTERNAL_INSPECTION_KIND";
/// Environment key carrying the per-request inspection token.
#[doc(hidden)]
pub const INSPECTION_TOKEN_ENV: &str = "MADS_INTERNAL_INSPECTION_TOKEN";
/// Environment key carrying the acknowledgement file path.
#[doc(hidden)]
pub const INSPECTION_ACK_ENV: &str = "MADS_INTERNAL_INSPECTION_ACK";
/// Environment key carrying the response file path.
#[doc(hidden)]
pub const INSPECTION_RESPONSE_ENV: &str = "MADS_INTERNAL_INSPECTION_RESPONSE";

/// A requested private inspection report kind.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionKind {
    /// Selected HTTP routes.
    Routes,
    /// The analyzed application graph.
    Graph,
    /// Grouped readiness checks.
    Doctor,
}

/// An owned source location carried by the private inspection transport.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceReport {
    /// Source file path.
    pub file: String,
    /// One-based source line.
    pub line: u32,
    /// One-based source column.
    pub column: u32,
}

/// An owned framework diagnostic carried by the private inspection transport.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticReport {
    /// Stable diagnostic code.
    pub code: String,
    /// Short diagnostic title.
    pub title: String,
    /// Detailed diagnostic message.
    pub message: String,
    /// Related subject when available.
    pub subject: Option<String>,
    /// Source location when available.
    pub location: Option<SourceReport>,
    /// Remediation suggestions.
    pub suggestions: Vec<String>,
}

/// A selected HTTP route carried by the private inspection transport.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RouteReport {
    /// Uppercase HTTP method.
    pub method: String,
    /// Complete route path.
    pub path: String,
    /// Route trait identity.
    pub route_trait: String,
    /// Route handler identity.
    pub handler: String,
    /// Controller identity.
    pub controller: String,
    /// Route declaration location.
    pub location: SourceReport,
    /// Whether an effective Passport guard is active.
    pub guard_active: bool,
}

/// A reachable application module carried by the private inspection transport.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModuleReport {
    /// Stable module type name.
    pub type_name: String,
    /// Module namespace.
    pub namespace: String,
    /// Module declaration location.
    pub location: SourceReport,
}

/// A direct reachable module import carried by the private inspection transport.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModuleImportReport {
    /// Importing module type name.
    pub importer: String,
    /// Imported module type name.
    pub imported: String,
}

/// A selected provider carried by the private inspection transport.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderReport {
    /// Stable provider type name.
    pub type_name: String,
    /// Owner module type name when module-owned.
    pub owner: Option<String>,
    /// Lowercase provider origin.
    pub origin: String,
    /// Lowercase provider visibility.
    pub visibility: String,
    /// Lowercase provider selection state.
    pub state: String,
    /// Provider declaration location when available.
    pub location: Option<SourceReport>,
}

/// A resolved provider dependency carried by the private inspection transport.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DependencyReport {
    /// Provider type name.
    pub provider: String,
    /// Required dependency type name.
    pub dependency: String,
}

/// Redacted configuration provenance carried by the private inspection transport.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConfigurationEvidenceReport {
    /// Configuration key.
    pub key: String,
    /// Winning source when available.
    pub source: Option<String>,
}

/// An auto-configuration evaluation carried by the private inspection transport.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutoConfigurationReport {
    /// Stable integration identifier.
    pub identifier: String,
    /// Auto-configured provider type name.
    pub output_type_name: String,
    /// Lowercase evaluation state.
    pub status: String,
    /// Stable reason code.
    pub reason_code: String,
    /// Redacted evaluation explanation.
    pub explanation: String,
    /// Redacted configuration provenance.
    pub configuration: Vec<ConfigurationEvidenceReport>,
}

/// One status for an inspection doctor check.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DoctorStatus {
    /// The group passed its required checks.
    Pass,
    /// The group does not apply to this selected application.
    Skipped,
    /// The group was deliberately supplied by application configuration.
    Overridden,
    /// The group has a required failure.
    Failed,
}

/// One grouped doctor result carried by the private inspection transport.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DoctorCheck {
    /// Stable doctor group name.
    pub group: String,
    /// Group outcome.
    pub status: DoctorStatus,
    /// Human-readable, redacted summary.
    pub summary: String,
}

impl DoctorCheck {
    /// Creates a passing doctor check.
    pub fn pass(group: impl Into<String>, summary: impl Into<String>) -> Self {
        Self::new(group, DoctorStatus::Pass, summary)
    }

    /// Creates a skipped doctor check.
    pub fn skipped(group: impl Into<String>, summary: impl Into<String>) -> Self {
        Self::new(group, DoctorStatus::Skipped, summary)
    }

    /// Creates an overridden doctor check.
    pub fn overridden(group: impl Into<String>, summary: impl Into<String>) -> Self {
        Self::new(group, DoctorStatus::Overridden, summary)
    }

    /// Creates a failing doctor check.
    pub fn failed(group: impl Into<String>, summary: impl Into<String>) -> Self {
        Self::new(group, DoctorStatus::Failed, summary)
    }

    fn new(group: impl Into<String>, status: DoctorStatus, summary: impl Into<String>) -> Self {
        Self {
            group: group.into(),
            status,
            summary: summary.into(),
        }
    }
}

/// Owned graph evidence carried by the private inspection transport.
#[doc(hidden)]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct GraphReport {
    /// Root module type name when analysis was rooted.
    pub root_module: Option<String>,
    /// Reachable modules.
    pub modules: Vec<ModuleReport>,
    /// Direct reachable module imports.
    pub imports: Vec<ModuleImportReport>,
    /// Selected providers.
    pub providers: Vec<ProviderReport>,
    /// Resolved provider dependencies.
    pub dependencies: Vec<DependencyReport>,
    /// Deterministic provider construction order when valid.
    pub construction_order: Option<Vec<String>>,
    /// Auto-configuration evaluations.
    pub auto_configurations: Vec<AutoConfigurationReport>,
}

/// One complete private inspection report.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InspectionReport {
    /// Requested report kind.
    pub kind: InspectionKind,
    /// Graph evidence.
    pub graph: GraphReport,
    /// Selected route evidence.
    pub routes: Vec<RouteReport>,
    /// Doctor checks.
    pub checks: Vec<DoctorCheck>,
    /// Framework diagnostics.
    pub diagnostics: Vec<DiagnosticReport>,
    /// Whether required inspection work failed.
    pub failed: bool,
}

/// A versioned, token-bound private inspection response.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InspectionEnvelope {
    protocol_version: u32,
    token: String,
    report: InspectionReport,
}

impl InspectionEnvelope {
    /// Creates a response using the current private transport version.
    pub fn new(token: String, report: InspectionReport) -> Self {
        Self {
            protocol_version: INSPECTION_PROTOCOL_VERSION,
            token,
            report,
        }
    }

    /// Returns the private transport version.
    pub const fn protocol_version(&self) -> u32 {
        self.protocol_version
    }

    /// Returns the response token.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Returns the assembled inspection report.
    pub const fn report(&self) -> &InspectionReport {
        &self.report
    }

    /// Consumes the envelope and returns its report.
    pub fn into_report(self) -> InspectionReport {
        self.report
    }
}

/// Assembles a side-effect-free report for a rooted standard HTTP application.
pub(crate) fn inspect_standard_application<M: Module>(
    root: &Path,
    kind: InspectionKind,
) -> InspectionReport {
    let (config, mut diagnostics, configuration_ok) = match load_standard_config_from(root) {
        Ok(config) => (config, Vec::new(), true),
        Err(error) => (
            Config::empty(),
            diagnostic_reports(error.diagnostics()),
            false,
        ),
    };

    let mut builder = Mads::builder_with_config(config);
    let root_ok = match builder.root::<M>() {
        Ok(_) => true,
        Err(error) => {
            diagnostics.extend(diagnostic_reports(error.diagnostics()));
            false
        }
    };
    let server_input_registered = enable_automatic_server(&mut builder);
    debug_assert!(server_input_registered);
    let cors_input_registered = enable_automatic_cors(&mut builder);
    debug_assert!(cors_input_registered);

    let analysis = builder.analyze();
    let snapshot = GraphInspectionSnapshot::from_analysis(&analysis);
    diagnostics.extend(diagnostic_reports(snapshot.diagnostics()));

    let (routes, route_ok, guard_ok) =
        match HttpApplicationScope::for_rooted_inspection(analysis.module_graph()) {
            Ok(scope) => inspect_scope(&scope, analysis.module_graph(), &mut diagnostics),
            Err(error) => {
                diagnostics.extend(diagnostic_reports(error.diagnostics()));
                (Vec::new(), false, false)
            }
        };

    diagnostics.sort_by(diagnostic_report_order);
    diagnostics.dedup();
    let graph = graph_report(&snapshot);
    let checks = doctor_checks(
        configuration_ok,
        root_ok,
        &snapshot,
        route_ok,
        guard_ok,
        &graph.auto_configurations,
    );
    let failed = !diagnostics.is_empty()
        || checks
            .iter()
            .any(|check| check.status == DoctorStatus::Failed);

    InspectionReport {
        kind,
        graph,
        routes,
        checks,
        diagnostics,
        failed,
    }
}

fn inspect_scope(
    scope: &HttpApplicationScope,
    _module_graph: Option<&ModuleGraph>,
    diagnostics: &mut Vec<DiagnosticReport>,
) -> (Vec<RouteReport>, bool, bool) {
    let mut routes = scope
        .route_records()
        .map(|(controller, contract, route)| RouteReport {
            method: route.method().as_str().to_owned(),
            path: route.full_path().to_owned(),
            route_trait: contract.trait_name().to_owned(),
            handler: route.handler().to_owned(),
            controller: controller.type_name().to_owned(),
            location: source_report(route.location()),
            #[cfg(feature = "jwt")]
            guard_active: route.guard().is_some(),
            #[cfg(not(feature = "jwt"))]
            guard_active: false,
        })
        .collect::<Vec<_>>();
    routes.sort_by(|left, right| {
        left.method
            .cmp(&right.method)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.controller.cmp(&right.controller))
            .then_with(|| left.route_trait.cmp(&right.route_trait))
            .then_with(|| left.handler.cmp(&right.handler))
    });

    let route_ok = match validate_scoped_descriptors(scope.controllers()) {
        Ok(_) => true,
        Err(error) => {
            diagnostics.extend(diagnostic_reports(error.diagnostics()));
            false
        }
    };
    #[cfg(feature = "jwt")]
    let guard_ok =
        match crate::PassportStrategyCatalog::preflight_scoped(_module_graph, scope.guards()) {
            Ok(_) => true,
            Err(error) => {
                diagnostics.extend(diagnostic_reports(error.diagnostics()));
                false
            }
        };
    #[cfg(not(feature = "jwt"))]
    let guard_ok = true;
    (routes, route_ok, guard_ok)
}

fn graph_report(snapshot: &GraphInspectionSnapshot) -> GraphReport {
    GraphReport {
        root_module: snapshot.root_module().map(str::to_owned),
        modules: snapshot
            .modules()
            .iter()
            .map(|module| ModuleReport {
                type_name: module.type_name().to_owned(),
                namespace: module.namespace().to_owned(),
                location: SourceReport {
                    file: module.location().file().to_owned(),
                    line: module.location().line(),
                    column: module.location().column(),
                },
            })
            .collect(),
        imports: snapshot
            .imports()
            .iter()
            .map(|import| ModuleImportReport {
                importer: import.importer().to_owned(),
                imported: import.imported().to_owned(),
            })
            .collect(),
        providers: snapshot
            .providers()
            .iter()
            .map(|provider| ProviderReport {
                type_name: provider.type_name().to_owned(),
                owner: provider.owner().map(str::to_owned),
                origin: provider_origin(provider.origin()).to_owned(),
                visibility: provider_visibility(provider.visibility()).to_owned(),
                state: provider_state(provider.state()).to_owned(),
                location: provider.location().map(|location| SourceReport {
                    file: location.file().to_owned(),
                    line: location.line(),
                    column: location.column(),
                }),
            })
            .collect(),
        dependencies: snapshot
            .dependencies()
            .iter()
            .map(|dependency| DependencyReport {
                provider: dependency.provider().to_owned(),
                dependency: dependency.dependency().to_owned(),
            })
            .collect(),
        construction_order: snapshot.construction_order().map(|order| order.to_vec()),
        auto_configurations: snapshot
            .auto_configurations()
            .iter()
            .map(|report| AutoConfigurationReport {
                identifier: report.identifier().to_owned(),
                output_type_name: report.output_type_name().to_owned(),
                status: auto_configuration_status(report.status()).to_owned(),
                reason_code: report.reason_code().to_owned(),
                explanation: report.explanation().to_owned(),
                configuration: report
                    .configuration()
                    .iter()
                    .map(|(key, source)| ConfigurationEvidenceReport {
                        key: key.to_owned(),
                        source: source.to_owned(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn doctor_checks(
    configuration_ok: bool,
    root_ok: bool,
    snapshot: &GraphInspectionSnapshot,
    route_ok: bool,
    guard_ok: bool,
    auto_configurations: &[AutoConfigurationReport],
) -> Vec<DoctorCheck> {
    let graph_ok = root_ok && snapshot.root_module().is_some();
    let providers_ok = snapshot.construction_order().is_some();
    let (server_status, server_summary) = auto_configuration_group(
        auto_configurations,
        &[SERVER_AUTO_CONFIGURATION_ID, CORS_AUTO_CONFIGURATION_ID],
        "no automatic server or CORS configuration applied",
    );
    let (auto_status, auto_summary) =
        auto_configuration_group(auto_configurations, &[], "no auto-configurations evaluated");
    vec![
        check_for(
            configuration_ok,
            "configuration",
            "conventional sources loaded",
            "conventional configuration could not be loaded",
        ),
        check_for(
            graph_ok,
            "module graph",
            "rooted module graph analyzed",
            "rooted module graph could not be analyzed",
        ),
        check_for(
            providers_ok,
            "providers",
            "provider graph has a construction plan",
            "provider graph has no construction plan",
        ),
        check_for(
            route_ok,
            "routes",
            "selected route metadata validated",
            "selected route metadata is invalid",
        ),
        guard_check(guard_ok),
        DoctorCheck {
            group: "server/CORS".into(),
            status: server_status,
            summary: server_summary,
        },
        DoctorCheck {
            group: "auto-configuration".into(),
            status: auto_status,
            summary: auto_summary,
        },
    ]
}

fn check_for(ok: bool, group: &str, success: &str, failure: &str) -> DoctorCheck {
    if ok {
        DoctorCheck::pass(group, success)
    } else {
        DoctorCheck::failed(group, failure)
    }
}

fn guard_check(_guard_ok: bool) -> DoctorCheck {
    #[cfg(feature = "jwt")]
    {
        check_for(
            _guard_ok,
            "guards/strategies",
            "guard and strategy metadata validated",
            "guard or strategy metadata is invalid",
        )
    }
    #[cfg(not(feature = "jwt"))]
    {
        DoctorCheck::skipped("guards/strategies", "JWT support is disabled")
    }
}

fn auto_configuration_group(
    reports: &[AutoConfigurationReport],
    identifiers: &[&str],
    empty_summary: &str,
) -> (DoctorStatus, String) {
    let selected = reports
        .iter()
        .filter(|report| {
            identifiers.is_empty() || identifiers.contains(&report.identifier.as_str())
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return (DoctorStatus::Skipped, empty_summary.into());
    }
    let status = if selected.iter().any(|report| report.status == "failed") {
        DoctorStatus::Failed
    } else if selected.iter().any(|report| report.status == "active") {
        DoctorStatus::Pass
    } else if selected.iter().any(|report| report.status == "overridden") {
        DoctorStatus::Overridden
    } else {
        DoctorStatus::Skipped
    };
    let summary = format!(
        "{} auto-configuration decision(s) evaluated",
        selected.len()
    );
    (status, summary)
}

fn source_report(location: mads_core::SourceLocation) -> SourceReport {
    SourceReport {
        file: location.file.to_owned(),
        line: location.line,
        column: location.column,
    }
}

fn diagnostic_reports(diagnostics: &[Diagnostic]) -> Vec<DiagnosticReport> {
    diagnostics.iter().map(diagnostic_report).collect()
}

fn diagnostic_report(diagnostic: &Diagnostic) -> DiagnosticReport {
    DiagnosticReport {
        code: diagnostic.code().as_str().to_owned(),
        title: diagnostic.title().to_owned(),
        message: diagnostic.message().to_owned(),
        subject: diagnostic.subject().map(str::to_owned),
        location: diagnostic.location().map(source_report),
        suggestions: diagnostic.suggestions().to_vec(),
    }
}

fn diagnostic_report_order(
    left: &DiagnosticReport,
    right: &DiagnosticReport,
) -> std::cmp::Ordering {
    left.code
        .cmp(&right.code)
        .then_with(|| left.title.cmp(&right.title))
        .then_with(|| left.message.cmp(&right.message))
        .then_with(|| left.subject.cmp(&right.subject))
        .then_with(|| {
            left.location
                .as_ref()
                .map(|location| (&location.file, location.line, location.column))
                .cmp(
                    &right
                        .location
                        .as_ref()
                        .map(|location| (&location.file, location.line, location.column)),
                )
        })
        .then_with(|| left.suggestions.cmp(&right.suggestions))
}

fn provider_origin(origin: ProviderOrigin) -> &'static str {
    match origin {
        ProviderOrigin::Provided => "provided",
        ProviderOrigin::AutoConfiguration => "auto_configuration",
        ProviderOrigin::Service => "service",
        ProviderOrigin::Repository => "repository",
        ProviderOrigin::Provider => "provider",
    }
}

fn provider_visibility(visibility: ProviderVisibility) -> &'static str {
    match visibility {
        ProviderVisibility::Public => "public",
        ProviderVisibility::Private => "private",
    }
}

fn provider_state(state: ProviderState) -> &'static str {
    match state {
        ProviderState::Provided => "provided",
        ProviderState::Preconstructed => "preconstructed",
        ProviderState::AutoConfigured => "auto_configured",
        ProviderState::Planned => "planned",
    }
}

fn auto_configuration_status(status: AutoConfigurationStatus) -> &'static str {
    match status {
        AutoConfigurationStatus::Active => "active",
        AutoConfigurationStatus::Skipped => "skipped",
        AutoConfigurationStatus::Overridden => "overridden",
        AutoConfigurationStatus::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);

    struct Marker;

    #[mads_core::provider]
    fn marker_provider() -> Marker {
        CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
        Marker
    }

    #[mads_core::module]
    struct AppModule;

    #[crate::routes]
    trait InspectionRoutes {
        #[crate::get("/inspection")]
        async fn inspection(&self) -> &'static str;
    }

    #[crate::controller(routes = [InspectionRoutes])]
    struct InspectionController;

    impl InspectionRoutes for InspectionController {
        async fn inspection(&self) -> &'static str {
            "inspection"
        }
    }

    #[test]
    fn report_analysis_does_not_construct_providers() {
        CONSTRUCTIONS.store(0, Ordering::SeqCst);
        let root = tempfile::tempdir().unwrap();

        let report = inspect_standard_application::<AppModule>(root.path(), InspectionKind::Doctor);

        assert!(!report.routes.is_empty());
        assert_eq!(CONSTRUCTIONS.load(Ordering::SeqCst), 0);
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.group == "server/CORS")
        );
    }

    #[test]
    fn report_keeps_configuration_values_redacted_and_is_deterministic() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("mads.toml"),
            "[database]\nurl = \"postgres://user:inspection-secret@localhost/db\"\n",
        )
        .unwrap();

        let first = inspect_standard_application::<AppModule>(root.path(), InspectionKind::Doctor);
        let second = inspect_standard_application::<AppModule>(root.path(), InspectionKind::Doctor);
        let json = serde_json::to_string(&first).unwrap();
        let debug = format!("{first:?}");

        assert_eq!(first, second);
        assert!(!json.contains("inspection-secret"));
        assert!(!debug.contains("inspection-secret"));
    }

    #[cfg(not(feature = "jwt"))]
    #[test]
    fn doctor_skips_guards_when_jwt_support_is_disabled() {
        let root = tempfile::tempdir().unwrap();
        let report = inspect_standard_application::<AppModule>(root.path(), InspectionKind::Doctor);

        assert!(report.checks.iter().any(|check| {
            check.group == "guards/strategies" && check.status == DoctorStatus::Skipped
        }));
    }
}

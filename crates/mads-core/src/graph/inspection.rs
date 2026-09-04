//! Owned, framework-neutral graph inspection snapshots.

use std::any::TypeId;
use std::collections::{BTreeMap, HashMap};

use crate::{
    AutoConfigurationStatus, Catalog, Diagnostic, ProviderOrigin, ProviderState,
    ProviderVisibility, SourceLocation,
};

use super::GraphAnalysis;

/// An owned source location retained for graph inspection.
#[doc(hidden)]
pub struct OwnedSourceLocation {
    file: String,
    line: u32,
    column: u32,
}

impl OwnedSourceLocation {
    /// Returns the source file path.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the one-based source line.
    pub const fn line(&self) -> u32 {
        self.line
    }

    /// Returns the one-based source column.
    pub const fn column(&self) -> u32 {
        self.column
    }
}

/// An owned reachable-module record retained for graph inspection.
#[doc(hidden)]
pub struct ModuleInspectionSnapshot {
    type_name: String,
    namespace: String,
    location: OwnedSourceLocation,
}

impl ModuleInspectionSnapshot {
    /// Returns the module's stable Rust type name.
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the Rust namespace owned by this module.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Returns the module declaration's source location.
    pub const fn location(&self) -> &OwnedSourceLocation {
        &self.location
    }
}

/// An owned direct module-import record retained for graph inspection.
#[doc(hidden)]
pub struct ModuleImportInspectionSnapshot {
    importer: String,
    imported: String,
}

impl ModuleImportInspectionSnapshot {
    /// Returns the importing module's stable type name.
    pub fn importer(&self) -> &str {
        &self.importer
    }

    /// Returns the imported module's stable type name.
    pub fn imported(&self) -> &str {
        &self.imported
    }
}

/// An owned provider record retained for graph inspection.
#[doc(hidden)]
pub struct ProviderInspectionSnapshot {
    type_name: String,
    owner: Option<String>,
    origin: ProviderOrigin,
    visibility: ProviderVisibility,
    state: ProviderState,
    location: Option<OwnedSourceLocation>,
}

impl ProviderInspectionSnapshot {
    /// Returns the provider's stable output type name.
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the owning module type name, when the provider is module-owned.
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    /// Returns how the provider enters the application graph.
    pub const fn origin(&self) -> ProviderOrigin {
        self.origin
    }

    /// Returns the provider declaration's visibility metadata.
    pub const fn visibility(&self) -> ProviderVisibility {
        self.visibility
    }

    /// Returns the provider's current satisfaction state.
    pub const fn state(&self) -> ProviderState {
        self.state
    }

    /// Returns the provider declaration's source location, when statically declared.
    pub const fn location(&self) -> Option<&OwnedSourceLocation> {
        match &self.location {
            Some(location) => Some(location),
            None => None,
        }
    }
}

/// An owned resolved dependency record retained for graph inspection.
#[doc(hidden)]
pub struct DependencyInspectionSnapshot {
    provider: String,
    dependency: String,
}

impl DependencyInspectionSnapshot {
    /// Returns the stable type name of the provider declaring the dependency.
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Returns the stable type name of the required dependency.
    pub fn dependency(&self) -> &str {
        &self.dependency
    }
}

/// An owned redacted auto-configuration record retained for graph inspection.
#[doc(hidden)]
pub struct AutoConfigurationInspectionSnapshot {
    identifier: String,
    output_type_name: String,
    status: AutoConfigurationStatus,
    reason_code: String,
    explanation: String,
    configuration: Vec<(String, Option<String>)>,
}

impl AutoConfigurationInspectionSnapshot {
    /// Returns the stable descriptor identifier.
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Returns the type name of the provider the descriptor can produce.
    pub fn output_type_name(&self) -> &str {
        &self.output_type_name
    }

    /// Returns the outcome of evaluating the descriptor.
    pub const fn status(&self) -> AutoConfigurationStatus {
        self.status
    }

    /// Returns the stable reason code for the outcome.
    pub fn reason_code(&self) -> &str {
        &self.reason_code
    }

    /// Returns the human-readable, redacted explanation.
    pub fn explanation(&self) -> &str {
        &self.explanation
    }

    /// Returns redacted configuration key and source evidence.
    pub fn configuration(&self) -> &[(String, Option<String>)] {
        &self.configuration
    }
}

/// An owned, deterministic snapshot of one graph analysis result.
#[doc(hidden)]
pub struct GraphInspectionSnapshot {
    root_module: Option<String>,
    modules: Vec<ModuleInspectionSnapshot>,
    imports: Vec<ModuleImportInspectionSnapshot>,
    providers: Vec<ProviderInspectionSnapshot>,
    dependencies: Vec<DependencyInspectionSnapshot>,
    construction_order: Option<Vec<String>>,
    auto_configurations: Vec<AutoConfigurationInspectionSnapshot>,
    diagnostics: Vec<Diagnostic>,
}

impl GraphInspectionSnapshot {
    /// Creates an owned snapshot from a graph analysis result.
    pub fn from_analysis(analysis: &GraphAnalysis) -> Self {
        let runtime_type_names = runtime_type_names();
        let module_graph = analysis.module_graph();
        let root_module = module_graph.map(|graph| graph.root().type_name().to_owned());
        let modules = module_graph.map_or_else(Vec::new, |graph| {
            graph
                .modules()
                .iter()
                .map(|module| ModuleInspectionSnapshot {
                    type_name: module.type_name().to_owned(),
                    namespace: module.namespace().to_owned(),
                    location: owned_location(module.location()),
                })
                .collect()
        });
        let imports = module_graph.map_or_else(Vec::new, |graph| {
            graph
                .imports()
                .iter()
                .map(|import| ModuleImportInspectionSnapshot {
                    importer: import.importer(graph).type_name().to_owned(),
                    imported: import.imported(graph).type_name().to_owned(),
                })
                .collect()
        });
        let owners = module_graph.map_or_else(BTreeMap::new, |graph| {
            graph
                .provider_ownership()
                .iter()
                .map(|ownership| (ownership.provider_type_name(), ownership.module_type_name()))
                .collect::<BTreeMap<_, _>>()
        });

        let mut providers = analysis
            .graph()
            .providers()
            .iter()
            .map(|provider| ProviderInspectionSnapshot {
                type_name: inspection_type_name(
                    &runtime_type_names,
                    provider.type_id,
                    provider.type_name(),
                ),
                owner: owners
                    .get(provider.type_name())
                    .copied()
                    .flatten()
                    .map(str::to_owned),
                origin: provider.origin(),
                visibility: provider.visibility(),
                state: provider.state(),
                location: provider.location().map(owned_location),
            })
            .collect::<Vec<_>>();
        providers.sort_by(|left, right| left.type_name.cmp(&right.type_name));

        let mut dependencies = analysis
            .graph()
            .dependencies()
            .iter()
            .map(|dependency| DependencyInspectionSnapshot {
                provider: inspection_type_name(
                    &runtime_type_names,
                    dependency.provider_type_id,
                    dependency.provider_type_name(),
                ),
                dependency: inspection_type_name(
                    &runtime_type_names,
                    dependency.dependency_type_id,
                    dependency.dependency_type_name(),
                ),
            })
            .collect::<Vec<_>>();
        dependencies.sort_by(|left, right| {
            left.provider
                .cmp(&right.provider)
                .then_with(|| left.dependency.cmp(&right.dependency))
        });

        let construction_order = analysis.construction_plan().map(|plan| {
            plan.steps()
                .iter()
                .map(|step| {
                    inspection_type_name(&runtime_type_names, step.type_id, step.type_name())
                })
                .collect()
        });

        let mut auto_configurations = analysis
            .auto_configurations()
            .iter()
            .map(|report| AutoConfigurationInspectionSnapshot {
                identifier: report.identifier().to_owned(),
                output_type_name: report.output_type_name().to_owned(),
                status: report.status(),
                reason_code: report.reason_code().as_str().to_owned(),
                explanation: report.explanation().to_owned(),
                configuration: report
                    .configuration()
                    .iter()
                    .map(|evidence| {
                        (
                            evidence.key().to_owned(),
                            evidence.source().map(str::to_owned),
                        )
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        auto_configurations.sort_by(|left, right| {
            left.identifier
                .cmp(&right.identifier)
                .then_with(|| left.output_type_name.cmp(&right.output_type_name))
                .then_with(|| left.reason_code.cmp(&right.reason_code))
                .then_with(|| left.explanation.cmp(&right.explanation))
        });

        let mut diagnostics = analysis.diagnostics().to_vec();
        diagnostics.sort_by(diagnostic_order);

        Self {
            root_module,
            modules,
            imports,
            providers,
            dependencies,
            construction_order,
            auto_configurations,
            diagnostics,
        }
    }

    /// Returns the selected root module's stable type name, when analysis was rooted.
    pub fn root_module(&self) -> Option<&str> {
        self.root_module.as_deref()
    }

    /// Returns reachable modules in authored traversal order.
    pub fn modules(&self) -> &[ModuleInspectionSnapshot] {
        &self.modules
    }

    /// Returns direct module imports in authored traversal order.
    pub fn imports(&self) -> &[ModuleImportInspectionSnapshot] {
        &self.imports
    }

    /// Returns providers in stable type-name order.
    pub fn providers(&self) -> &[ProviderInspectionSnapshot] {
        &self.providers
    }

    /// Returns resolved dependencies in stable provider and dependency order.
    pub fn dependencies(&self) -> &[DependencyInspectionSnapshot] {
        &self.dependencies
    }

    /// Returns the deterministic provider construction order, when analysis is valid.
    pub fn construction_order(&self) -> Option<&[String]> {
        self.construction_order.as_deref()
    }

    /// Returns auto-configuration reports in stable order.
    pub fn auto_configurations(&self) -> &[AutoConfigurationInspectionSnapshot] {
        &self.auto_configurations
    }

    /// Returns copied diagnostics in stable report order.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

fn runtime_type_names() -> HashMap<TypeId, &'static str> {
    Catalog::providers()
        .into_iter()
        .filter_map(|descriptor| {
            descriptor
                .runtime_type_name()
                .map(|type_name| (descriptor.type_id(), type_name))
        })
        .collect()
}

fn inspection_type_name(
    runtime_type_names: &HashMap<TypeId, &'static str>,
    type_id: TypeId,
    fallback: &str,
) -> String {
    runtime_type_names
        .get(&type_id)
        .copied()
        .unwrap_or(fallback)
        .to_owned()
}

fn owned_location(location: SourceLocation) -> OwnedSourceLocation {
    OwnedSourceLocation {
        file: location.file.to_owned(),
        line: location.line,
        column: location.column,
    }
}

fn diagnostic_order(left: &Diagnostic, right: &Diagnostic) -> std::cmp::Ordering {
    left.code()
        .as_str()
        .cmp(right.code().as_str())
        .then_with(|| left.title().cmp(right.title()))
        .then_with(|| left.message().cmp(right.message()))
        .then_with(|| left.subject().cmp(&right.subject()))
        .then_with(|| location_order(left.location(), right.location()))
        .then_with(|| left.suggestions().cmp(right.suggestions()))
}

fn location_order(
    left: Option<SourceLocation>,
    right: Option<SourceLocation>,
) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left
            .file
            .cmp(right.file)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.column.cmp(&right.column)),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

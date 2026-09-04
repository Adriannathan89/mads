//! Immutable provider graph inspection types.

use crate::Catalog;

mod analysis;
mod cycle;
mod inspection;
mod model;
mod module;
mod plan;
mod scope;

#[doc(hidden)]
pub use inspection::{
    AutoConfigurationInspectionSnapshot, DependencyInspectionSnapshot, GraphInspectionSnapshot,
    ModuleImportInspectionSnapshot, ModuleInspectionSnapshot, OwnedSourceLocation,
    ProviderInspectionSnapshot,
};
pub use model::{
    ApplicationGraph, ConstructionPlan, ConstructionStep, DependencyEdge, GraphAnalysis,
    ProviderNode, ProviderOrigin, ProviderState,
};
pub use module::{ModuleGraph, ModuleImportEdge, ModuleNode, ProviderOwnership};

pub(crate) use model::SatisfiedProvider;
pub(crate) use module::{build_module_graph, validate_module_catalog};
pub(crate) use scope::select_scoped_providers;

pub(crate) fn analyze_descriptors(
    descriptors: &[&'static crate::ProviderDescriptor],
    satisfied: &[SatisfiedProvider],
    covered_missing: &[std::any::TypeId],
) -> GraphAnalysis {
    analysis::analyze_parts(descriptors, satisfied, covered_missing)
}

pub(crate) fn analyze_catalog(
    satisfied: &[SatisfiedProvider],
    covered_missing: &[std::any::TypeId],
) -> GraphAnalysis {
    let descriptors = Catalog::providers();
    analyze_descriptors(&descriptors, satisfied, covered_missing)
}

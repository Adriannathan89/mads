//! Immutable provider graph inspection types.

use crate::Catalog;

mod analysis;
mod cycle;
mod model;
mod module;
mod plan;
mod scope;

pub use model::{
    ApplicationGraph, ConstructionPlan, ConstructionStep, DependencyEdge, GraphAnalysis,
    ProviderNode, ProviderOrigin, ProviderState,
};
pub use module::{ModuleGraph, ModuleImportEdge, ModuleNode, ProviderOwnership};

pub(crate) use model::SatisfiedProvider;
pub(crate) use module::build_module_graph;
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

pub(crate) fn analyze_module_scope<M: crate::Module>() -> crate::Result<GraphAnalysis> {
    let modules = Catalog::modules();
    let mut module_graph = build_module_graph(std::any::TypeId::of::<M>(), &modules)?;
    let descriptors = Catalog::providers();
    let scoped = select_scoped_providers(&module_graph, &descriptors, &[]);
    let mut analysis = analyze_descriptors(&scoped.descriptors, &[], &scoped.covered_missing);
    analysis.prepend_diagnostics(scoped.diagnostics);
    module_graph.set_provider_ownership(scoped.ownership);
    Ok(analysis)
}

//! Immutable provider graph inspection types.

use crate::Catalog;

mod analysis;
mod cycle;
mod model;
mod plan;

pub use model::{
    ApplicationGraph, ConstructionPlan, ConstructionStep, DependencyEdge, GraphAnalysis,
    ProviderNode, ProviderOrigin, ProviderState,
};

pub(crate) use model::SatisfiedProvider;

pub(crate) fn analyze_catalog(
    satisfied: &[SatisfiedProvider],
    covered_missing: &[std::any::TypeId],
) -> GraphAnalysis {
    let descriptors = Catalog::providers();
    analysis::analyze_parts(&descriptors, satisfied, covered_missing)
}

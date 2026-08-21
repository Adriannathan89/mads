//! Integration tests for public graph analysis and inspection.

use mads_core::{ConstructionStep, Mads, ProviderOrigin, ProviderState};

#[derive(Clone)]
struct GraphDatabase;

struct GraphRepository {
    _database: GraphDatabase,
}

#[mads_core::provider]
fn graph_database() -> GraphDatabase {
    GraphDatabase
}

#[mads_core::provider]
fn graph_repository(database: GraphDatabase) -> GraphRepository {
    GraphRepository {
        _database: database,
    }
}

#[test]
fn analysis_exposes_nodes_edges_and_a_dependency_ordered_plan() {
    let analysis = Mads::builder().analyze();
    assert!(analysis.is_valid());
    assert_eq!(
        analysis
            .graph()
            .provider::<GraphDatabase>()
            .unwrap()
            .state(),
        ProviderState::Planned
    );
    assert_eq!(
        analysis
            .graph()
            .provider::<GraphRepository>()
            .unwrap()
            .origin(),
        ProviderOrigin::Provider
    );
    assert!(analysis.graph().dependencies().iter().any(|edge| {
        edge.provider_type_name().contains("GraphRepository")
            && edge.dependency_type_name().contains("GraphDatabase")
    }));
    assert_eq!(
        analysis
            .construction_plan()
            .unwrap()
            .steps()
            .iter()
            .map(ConstructionStep::type_name)
            .collect::<Vec<_>>(),
        ["GraphDatabase", "GraphRepository"],
    );
}

//! Deterministic construction planning for validated provider graphs.

use std::collections::BTreeSet;

use crate::ProviderDescriptor;

use super::model::{
    ApplicationGraph, ConstructionPlan, ConstructionStep, ProviderNode, ProviderState,
};

pub(crate) fn create(
    graph: &ApplicationGraph,
    descriptors: &[&'static ProviderDescriptor],
) -> ConstructionPlan {
    let mut remaining_dependencies = vec![0_usize; graph.providers.len()];
    let mut consumers = vec![Vec::new(); graph.providers.len()];

    for edge in &graph.dependencies {
        let provider = graph
            .providers
            .iter()
            .position(|node| node.type_id == edge.provider_type_id)
            .expect("dependency edges reference graph providers");
        let dependency = graph
            .providers
            .iter()
            .position(|node| node.type_id == edge.dependency_type_id)
            .expect("dependency edges reference graph dependencies");
        if graph.providers[provider].state == ProviderState::Planned
            && graph.providers[dependency].state == ProviderState::Planned
        {
            remaining_dependencies[provider] += 1;
            consumers[dependency].push(provider);
        }
    }

    let planned_count = graph
        .providers
        .iter()
        .filter(|provider| provider.state == ProviderState::Planned)
        .count();
    let mut ready = BTreeSet::new();
    for (index, provider) in graph.providers.iter().enumerate() {
        if provider.state == ProviderState::Planned && remaining_dependencies[index] == 0 {
            ready.insert(ReadyNode::new(index, provider));
        }
    }

    let mut steps = Vec::with_capacity(planned_count);
    while let Some(ready_node) = ready.pop_first() {
        let provider = &graph.providers[ready_node.index];
        let descriptor = descriptors
            .iter()
            .copied()
            .find(|descriptor| descriptor.type_id() == provider.type_id)
            .expect("planned graph providers have descriptors");
        steps.push(ConstructionStep {
            type_id: provider.type_id,
            type_name: provider.type_name,
            origin: provider.origin,
            location: provider.location.expect("planned providers have locations"),
            descriptor,
        });

        for consumer in &consumers[ready_node.index] {
            remaining_dependencies[*consumer] -= 1;
            if remaining_dependencies[*consumer] == 0 {
                ready.insert(ReadyNode::new(*consumer, &graph.providers[*consumer]));
            }
        }
    }

    assert_eq!(
        steps.len(),
        planned_count,
        "cycle validation must precede planning"
    );
    ConstructionPlan { steps }
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct ReadyNode {
    type_name: &'static str,
    origin: super::model::ProviderOrigin,
    file: &'static str,
    line: u32,
    column: u32,
    index: usize,
}

impl ReadyNode {
    fn new(index: usize, provider: &ProviderNode) -> Self {
        let (file, line, column) = match provider.location {
            Some(location) => (location.file, location.line, location.column),
            None => ("", 0, 0),
        };
        Self {
            type_name: provider.type_name,
            origin: provider.origin,
            file,
            line,
            column,
            index,
        }
    }
}

//! Canonical cycle detection for resolved provider graphs.

use std::cmp::Ordering;

use crate::SourceLocation;

use super::model::ApplicationGraph;

pub(crate) struct Cycle {
    pub(crate) names: Vec<&'static str>,
    pub(crate) location: SourceLocation,
}

pub(crate) fn detect(graph: &ApplicationGraph) -> Vec<Cycle> {
    let mut colors = vec![Color::White; graph.providers.len()];
    let mut stack = Vec::new();
    let mut cycles = Vec::new();

    for index in 0..graph.providers.len() {
        if colors[index] == Color::White {
            visit(index, graph, &mut colors, &mut stack, &mut cycles);
        }
    }

    cycles.sort_by(|left, right| left.names.cmp(&right.names));
    cycles.dedup_by(|left, right| left.names == right.names);
    cycles
}

fn visit(
    index: usize,
    graph: &ApplicationGraph,
    colors: &mut [Color],
    stack: &mut Vec<usize>,
    cycles: &mut Vec<Cycle>,
) {
    colors[index] = Color::Gray;
    stack.push(index);

    for dependency in outgoing(graph, index) {
        match colors[dependency] {
            Color::White => visit(dependency, graph, colors, stack, cycles),
            Color::Gray => {
                let cycle_start = stack
                    .iter()
                    .position(|candidate| *candidate == dependency)
                    .expect("gray providers are always active");
                let open_cycle = &stack[cycle_start..];
                let canonical = canonicalize(graph, open_cycle);
                let mut names = canonical
                    .iter()
                    .map(|candidate| graph.providers[*candidate].type_name)
                    .collect::<Vec<_>>();
                names.push(names[0]);
                let location = graph.providers[canonical[0]]
                    .location
                    .expect("cycle providers are statically declared");
                cycles.push(Cycle { names, location });
            }
            Color::Black => {}
        }
    }

    stack.pop();
    colors[index] = Color::Black;
}

fn outgoing(graph: &ApplicationGraph, provider: usize) -> Vec<usize> {
    let provider_type_id = graph.providers[provider].type_id;
    let mut dependencies = graph
        .dependencies
        .iter()
        .filter(|edge| edge.provider_type_id == provider_type_id)
        .filter_map(|edge| {
            graph
                .providers
                .iter()
                .position(|node| node.type_id == edge.dependency_type_id)
        })
        .collect::<Vec<_>>();
    dependencies.sort_by(|left, right| node_order(graph, *left, *right));
    dependencies
}

fn canonicalize(graph: &ApplicationGraph, open_cycle: &[usize]) -> Vec<usize> {
    let start = open_cycle
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| node_order(graph, **left, **right))
        .expect("cycles are non-empty")
        .0;
    open_cycle[start..]
        .iter()
        .chain(&open_cycle[..start])
        .copied()
        .collect()
}

fn node_order(graph: &ApplicationGraph, left: usize, right: usize) -> Ordering {
    let left = &graph.providers[left];
    let right = &graph.providers[right];
    left.type_name
        .cmp(right.type_name)
        .then_with(|| left.origin.cmp(&right.origin))
        .then_with(|| match (left.location, right.location) {
            (Some(left), Some(right)) => left
                .file
                .cmp(right.file)
                .then_with(|| left.line.cmp(&right.line))
                .then_with(|| left.column.cmp(&right.column)),
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Color {
    White,
    Gray,
    Black,
}

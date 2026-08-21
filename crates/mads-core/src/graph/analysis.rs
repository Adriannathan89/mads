//! Deterministic validation of provider descriptors and dependency edges.

#![allow(dead_code)]

use std::cmp::Ordering;

use crate::{
    Diagnostic, MADS001, MADS002, MADS003, MADS005, ProviderDescriptor, ProviderOrigin,
    ProviderState, ProviderVisibility, SourceLocation,
};

use super::model::{
    ApplicationGraph, DependencyEdge, GraphAnalysis, ProviderNode, SatisfiedProvider,
};
use super::{cycle, plan};

pub(crate) fn analyze_parts(
    descriptors: &[&'static ProviderDescriptor],
    satisfied: &[SatisfiedProvider],
) -> GraphAnalysis {
    let mut descriptors = descriptors.to_vec();
    descriptors.sort_by(|left, right| descriptor_order(left, right));

    let mut satisfied = satisfied.to_vec();
    satisfied.sort_by(|left, right| {
        left.type_name
            .cmp(right.type_name)
            .then_with(|| provider_state_order(left.state).cmp(&provider_state_order(right.state)))
    });

    let mut diagnostics = Vec::new();
    let mut unique_descriptors: Vec<&'static ProviderDescriptor> = Vec::new();
    let mut duplicate_identities: Vec<&'static ProviderDescriptor> = Vec::new();
    for descriptor in descriptors {
        if let Some(existing) = unique_descriptors
            .iter()
            .copied()
            .find(|unique| exact_identity(unique, descriptor))
        {
            if !duplicate_identities
                .iter()
                .any(|duplicate| exact_identity(duplicate, existing))
            {
                diagnostics.push(PendingDiagnostic::duplicate(existing));
                duplicate_identities.push(existing);
            }
        } else {
            unique_descriptors.push(descriptor);
        }
    }

    let mut ambiguous_types = Vec::new();
    for descriptor in &unique_descriptors {
        let same_type: Vec<_> = unique_descriptors
            .iter()
            .copied()
            .filter(|candidate| candidate.type_id() == descriptor.type_id())
            .collect();
        if same_type.len() > 1
            && !ambiguous_types
                .iter()
                .any(|type_id| *type_id == descriptor.type_id())
        {
            diagnostics.push(PendingDiagnostic::ambiguous(same_type[0]));
            ambiguous_types.push(descriptor.type_id());
        }
    }

    let mut providers = Vec::new();
    for satisfied_provider in &satisfied {
        if ambiguous_types.contains(&satisfied_provider.type_id) {
            continue;
        }
        if providers
            .iter()
            .any(|provider: &ProviderNode| provider.type_id == satisfied_provider.type_id)
        {
            continue;
        }

        let matching_descriptor = unique_descriptors
            .iter()
            .copied()
            .find(|descriptor| descriptor.type_id() == satisfied_provider.type_id);
        let origin = match (satisfied_provider.state, matching_descriptor) {
            (ProviderState::Provided, _) | (_, None) => ProviderOrigin::Provided,
            (_, Some(descriptor)) => descriptor.kind().into(),
        };
        providers.push(ProviderNode {
            type_id: satisfied_provider.type_id,
            type_name: satisfied_provider.type_name,
            origin,
            visibility: ProviderVisibility::Public,
            state: satisfied_provider.state,
            location: None,
            declared_dependencies: &[],
        });
    }

    for &descriptor in &unique_descriptors {
        let type_id = descriptor.type_id();
        if ambiguous_types.contains(&type_id)
            || providers.iter().any(|provider| provider.type_id == type_id)
        {
            continue;
        }

        providers.push(ProviderNode {
            type_id,
            type_name: descriptor.type_name(),
            origin: descriptor.kind().into(),
            visibility: descriptor.visibility(),
            state: ProviderState::Planned,
            location: Some(descriptor.location()),
            declared_dependencies: descriptor.dependencies(),
        });
    }
    providers.sort_by(|left, right| left.type_name.cmp(right.type_name));

    let mut dependencies = Vec::new();
    for provider in &providers {
        if provider.state != ProviderState::Planned {
            continue;
        }

        for dependency in provider.declared_dependencies {
            let dependency_type_id = dependency.type_id();
            if ambiguous_types.contains(&dependency_type_id) {
                continue;
            }
            if providers
                .iter()
                .any(|candidate| candidate.type_id == dependency_type_id)
            {
                dependencies.push(DependencyEdge {
                    provider_type_id: provider.type_id,
                    provider_type_name: provider.type_name,
                    dependency_type_id,
                    dependency_type_name: dependency.type_name(),
                });
            } else {
                diagnostics.push(PendingDiagnostic::missing(provider, dependency));
            }
        }
    }

    let graph = ApplicationGraph {
        providers,
        dependencies,
    };
    if diagnostics.is_empty() {
        for cycle in cycle::detect(&graph) {
            diagnostics.push(PendingDiagnostic::cycle(cycle));
        }
    }

    diagnostics.sort_by(PendingDiagnostic::order);
    let diagnostics = diagnostics
        .into_iter()
        .map(|pending| pending.diagnostic)
        .collect::<Vec<_>>();
    let construction_plan = diagnostics
        .is_empty()
        .then(|| plan::create(&graph, &unique_descriptors));

    GraphAnalysis {
        graph,
        diagnostics,
        construction_plan,
    }
}

fn descriptor_order(left: &ProviderDescriptor, right: &ProviderDescriptor) -> Ordering {
    left.type_name()
        .cmp(right.type_name())
        .then_with(|| left.kind().cmp(&right.kind()))
        .then_with(|| location_order(left.location(), right.location()))
}

fn exact_identity(left: &ProviderDescriptor, right: &ProviderDescriptor) -> bool {
    left.type_id() == right.type_id()
        && left.kind() == right.kind()
        && left.type_name() == right.type_name()
        && left.visibility() == right.visibility()
        && left.location() == right.location()
        && left.dependencies().len() == right.dependencies().len()
        && left
            .dependencies()
            .iter()
            .zip(right.dependencies())
            .all(|(left, right)| left.type_id() == right.type_id())
}

fn provider_state_order(state: ProviderState) -> u8 {
    match state {
        ProviderState::Provided => 0,
        ProviderState::Preconstructed => 1,
        ProviderState::Planned => 2,
    }
}

fn location_order(left: SourceLocation, right: SourceLocation) -> Ordering {
    left.file
        .cmp(right.file)
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.column.cmp(&right.column))
}

struct PendingDiagnostic {
    diagnostic: Diagnostic,
    subject: &'static str,
    location: Option<SourceLocation>,
}

impl PendingDiagnostic {
    fn duplicate(descriptor: &'static ProviderDescriptor) -> Self {
        Self {
            diagnostic: Diagnostic::new(
                MADS001,
                "duplicate provider declaration",
                "the same provider declaration was registered more than once",
            )
            .with_subject(descriptor.type_name())
            .with_location(descriptor.location())
            .with_suggestion("remove the repeated provider declaration"),
            subject: descriptor.type_name(),
            location: Some(descriptor.location()),
        }
    }

    fn ambiguous(descriptor: &'static ProviderDescriptor) -> Self {
        Self {
            diagnostic: Diagnostic::new(
                MADS002,
                "ambiguous provider binding",
                "multiple provider declarations produce the same concrete type",
            )
            .with_subject(descriptor.type_name())
            .with_location(descriptor.location())
            .with_suggestion("remove one provider declaration or change its output type"),
            subject: descriptor.type_name(),
            location: Some(descriptor.location()),
        }
    }

    fn missing(provider: &ProviderNode, dependency: &crate::DependencyDescriptor) -> Self {
        Self {
            diagnostic: Diagnostic::new(
                MADS003,
                "unresolved dependency",
                format!(
                    "{} requires {}, but no provider exists for that type",
                    provider.type_name,
                    dependency.type_name(),
                ),
            )
            .with_subject(dependency.type_name())
            .with_location(
                provider
                    .location
                    .expect("planned providers have a location"),
            )
            .with_suggestion("register a provider or explicitly provide the missing dependency"),
            subject: dependency.type_name(),
            location: provider.location,
        }
    }

    fn cycle(cycle: cycle::Cycle) -> Self {
        let subject = cycle.names[0];
        let message = format!("provider dependency cycle: {}", cycle.names.join(" -> "));
        Self {
            diagnostic: Diagnostic::new(MADS005, "dependency cycle", message)
                .with_subject(subject)
                .with_location(cycle.location)
                .with_suggestion("remove or invert one dependency in the cycle"),
            subject,
            location: Some(cycle.location),
        }
    }

    fn order(left: &Self, right: &Self) -> Ordering {
        left.diagnostic
            .code()
            .as_str()
            .cmp(right.diagnostic.code().as_str())
            .then_with(|| left.subject.cmp(right.subject))
            .then_with(|| match (left.location, right.location) {
                (Some(left), Some(right)) => location_order(left, right),
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            })
    }
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::sync::Arc;

    use crate::{
        ConstructionContext, ConstructionStep, DependencyDescriptor, Diagnostic, ErasedProvider,
        MADS001, MADS002, MADS003, MADS005, ProviderDescriptor, ProviderFuture, ProviderKind,
        ProviderOrigin, ProviderState, ProviderVisibility, SourceLocation,
    };

    use super::*;

    #[derive(Default)]
    struct Database;
    #[derive(Default)]
    struct FirstMissing;
    #[derive(Default)]
    struct SecondMissing;
    struct AbsentAlpha;
    struct AbsentZeta;
    #[derive(Default)]
    struct SelfCycle;
    #[derive(Default)]
    struct A;
    #[derive(Default)]
    struct B;
    #[derive(Default)]
    struct C;
    #[derive(Default)]
    struct Alpha;
    #[derive(Default)]
    struct PlanDatabase;
    #[derive(Default)]
    struct Service;
    #[derive(Default)]
    struct Zeta;

    fn database_type_id() -> TypeId {
        TypeId::of::<Database>()
    }

    fn first_missing_type_id() -> TypeId {
        TypeId::of::<FirstMissing>()
    }

    fn second_missing_type_id() -> TypeId {
        TypeId::of::<SecondMissing>()
    }

    fn absent_alpha_type_id() -> TypeId {
        TypeId::of::<AbsentAlpha>()
    }

    fn absent_zeta_type_id() -> TypeId {
        TypeId::of::<AbsentZeta>()
    }

    fn self_cycle_type_id() -> TypeId {
        TypeId::of::<SelfCycle>()
    }

    fn a_type_id() -> TypeId {
        TypeId::of::<A>()
    }

    fn b_type_id() -> TypeId {
        TypeId::of::<B>()
    }

    fn c_type_id() -> TypeId {
        TypeId::of::<C>()
    }

    fn alpha_type_id() -> TypeId {
        TypeId::of::<Alpha>()
    }

    fn plan_database_type_id() -> TypeId {
        TypeId::of::<PlanDatabase>()
    }

    fn service_type_id() -> TypeId {
        TypeId::of::<Service>()
    }

    fn zeta_type_id() -> TypeId {
        TypeId::of::<Zeta>()
    }

    fn constructor<'a, T>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a>
    where
        T: Default + Send + Sync + 'static,
    {
        Box::pin(async { Ok(Arc::new(T::default()) as ErasedProvider) })
    }

    static FIRST_MISSING_DEPENDENCIES: [DependencyDescriptor; 1] = [DependencyDescriptor::new(
        "fixture::AbsentZeta",
        absent_zeta_type_id,
    )];
    static SECOND_MISSING_DEPENDENCIES: [DependencyDescriptor; 1] = [DependencyDescriptor::new(
        "fixture::AbsentAlpha",
        absent_alpha_type_id,
    )];
    static SELF_DEPENDENCIES: [DependencyDescriptor; 1] = [DependencyDescriptor::new(
        "cycle::SelfCycle",
        self_cycle_type_id,
    )];
    static A_DEPENDENCIES: [DependencyDescriptor; 1] =
        [DependencyDescriptor::new("cycle::B", b_type_id)];
    static B_DEPENDENCIES: [DependencyDescriptor; 1] =
        [DependencyDescriptor::new("cycle::C", c_type_id)];
    static C_DEPENDENCIES: [DependencyDescriptor; 1] =
        [DependencyDescriptor::new("cycle::A", a_type_id)];
    static SERVICE_DEPENDENCIES: [DependencyDescriptor; 1] = [DependencyDescriptor::new(
        "plan::Database",
        plan_database_type_id,
    )];

    static DATABASE_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
        ProviderKind::Provider,
        "fixture::Database",
        database_type_id,
        &[],
        ProviderVisibility::Private,
        SourceLocation::new("database.rs", 1, 1),
        constructor::<Database>,
    );
    static ALTERNATE_DATABASE_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
        ProviderKind::Provider,
        "fixture::AlternateDatabase",
        database_type_id,
        &[],
        ProviderVisibility::Private,
        SourceLocation::new("alternate.rs", 1, 1),
        constructor::<Database>,
    );
    static FIRST_MISSING_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
        ProviderKind::Service,
        "fixture::FirstMissing",
        first_missing_type_id,
        &FIRST_MISSING_DEPENDENCIES,
        ProviderVisibility::Private,
        SourceLocation::new("first.rs", 1, 1),
        constructor::<FirstMissing>,
    );
    static SECOND_MISSING_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
        ProviderKind::Service,
        "fixture::SecondMissing",
        second_missing_type_id,
        &SECOND_MISSING_DEPENDENCIES,
        ProviderVisibility::Private,
        SourceLocation::new("second.rs", 1, 1),
        constructor::<SecondMissing>,
    );

    macro_rules! test_descriptor {
        ($static_name:ident, $type:ty, $stable_name:literal, $type_id:ident, $dependencies:expr, $file:literal) => {
            static $static_name: ProviderDescriptor = ProviderDescriptor::new(
                ProviderKind::Provider,
                $stable_name,
                $type_id,
                $dependencies,
                ProviderVisibility::Private,
                SourceLocation::new($file, 1, 1),
                constructor::<$type>,
            );
        };
    }

    test_descriptor!(
        SELF_CYCLE_DESCRIPTOR,
        SelfCycle,
        "cycle::SelfCycle",
        self_cycle_type_id,
        &SELF_DEPENDENCIES,
        "cycle_SelfCycle.rs"
    );
    test_descriptor!(
        A_DESCRIPTOR,
        A,
        "cycle::A",
        a_type_id,
        &A_DEPENDENCIES,
        "cycle_A.rs"
    );
    test_descriptor!(
        B_DESCRIPTOR,
        B,
        "cycle::B",
        b_type_id,
        &B_DEPENDENCIES,
        "cycle_B.rs"
    );
    test_descriptor!(
        C_DESCRIPTOR,
        C,
        "cycle::C",
        c_type_id,
        &C_DEPENDENCIES,
        "cycle_C.rs"
    );
    test_descriptor!(
        ALPHA_DESCRIPTOR,
        Alpha,
        "plan::Alpha",
        alpha_type_id,
        &[],
        "plan_Alpha.rs"
    );
    test_descriptor!(
        PLAN_DATABASE_DESCRIPTOR,
        PlanDatabase,
        "plan::Database",
        plan_database_type_id,
        &[],
        "plan_Database.rs"
    );
    test_descriptor!(
        SERVICE_DESCRIPTOR,
        Service,
        "plan::Service",
        service_type_id,
        &SERVICE_DEPENDENCIES,
        "plan_Service.rs"
    );
    test_descriptor!(
        ZETA_DESCRIPTOR,
        Zeta,
        "plan::Zeta",
        zeta_type_id,
        &[],
        "plan_Zeta.rs"
    );

    #[test]
    fn provided_values_override_one_descriptor_and_remain_public() {
        let satisfied = [SatisfiedProvider::provided::<Database>()];
        let analysis = analyze_parts(&[&DATABASE_DESCRIPTOR], &satisfied);

        assert!(analysis.is_valid());
        let node = analysis.graph().provider::<Database>().unwrap();
        assert_eq!(node.origin(), ProviderOrigin::Provided);
        assert_eq!(node.visibility(), ProviderVisibility::Public);
        assert_eq!(node.state(), ProviderState::Provided);
        assert!(analysis.construction_plan().unwrap().steps().is_empty());
    }

    #[test]
    fn different_providers_for_one_type_are_ambiguous_even_when_provided() {
        let satisfied = [SatisfiedProvider::provided::<Database>()];
        let analysis = analyze_parts(
            &[&DATABASE_DESCRIPTOR, &ALTERNATE_DATABASE_DESCRIPTOR],
            &satisfied,
        );

        assert_eq!(analysis.diagnostics().len(), 1);
        assert_eq!(analysis.diagnostics()[0].code(), MADS002);
        assert!(analysis.graph().provider::<Database>().is_none());
        assert!(analysis.construction_plan().is_none());
    }

    #[test]
    fn repeated_exact_identity_is_duplicate_not_ambiguous() {
        let analysis = analyze_parts(&[&DATABASE_DESCRIPTOR, &DATABASE_DESCRIPTOR], &[]);

        assert_eq!(analysis.diagnostics().len(), 1);
        assert_eq!(analysis.diagnostics()[0].code(), MADS001);
    }

    #[test]
    fn repeated_identity_group_emits_one_duplicate_diagnostic() {
        let analysis = analyze_parts(
            &[
                &DATABASE_DESCRIPTOR,
                &DATABASE_DESCRIPTOR,
                &DATABASE_DESCRIPTOR,
            ],
            &[],
        );

        assert_eq!(analysis.diagnostics().len(), 1);
        assert_eq!(analysis.diagnostics()[0].code(), MADS001);
    }

    #[test]
    fn unresolved_dependencies_are_aggregated_in_stable_type_order() {
        let analysis = analyze_parts(
            &[&FIRST_MISSING_DESCRIPTOR, &SECOND_MISSING_DESCRIPTOR],
            &[],
        );
        let codes: Vec<_> = analysis
            .diagnostics()
            .iter()
            .map(Diagnostic::code)
            .collect();

        assert_eq!(codes, [MADS003, MADS003]);
        assert!(
            analysis.diagnostics()[0]
                .to_string()
                .contains("AbsentAlpha")
        );
        assert!(analysis.diagnostics()[1].to_string().contains("AbsentZeta"));
        assert!(analysis.construction_plan().is_none());
    }

    #[test]
    fn self_cycle_renders_a_closed_path() {
        let analysis = analyze_parts(&[&SELF_CYCLE_DESCRIPTOR], &[]);

        assert_eq!(analysis.diagnostics()[0].code(), MADS005);
        assert!(
            analysis.diagnostics()[0]
                .to_string()
                .contains("cycle::SelfCycle -> cycle::SelfCycle")
        );
    }

    #[test]
    fn multi_node_cycle_is_canonical_regardless_of_input_order() {
        let first = analyze_parts(&[&C_DESCRIPTOR, &A_DESCRIPTOR, &B_DESCRIPTOR], &[]);
        let second = analyze_parts(&[&B_DESCRIPTOR, &C_DESCRIPTOR, &A_DESCRIPTOR], &[]);

        assert_eq!(first.diagnostics(), second.diagnostics());
        assert!(
            first.diagnostics()[0]
                .to_string()
                .contains("cycle::A -> cycle::B -> cycle::C -> cycle::A")
        );
    }

    #[test]
    fn independent_nodes_use_stable_topological_tie_breaking() {
        let analysis = analyze_parts(
            &[
                &SERVICE_DESCRIPTOR,
                &ZETA_DESCRIPTOR,
                &PLAN_DATABASE_DESCRIPTOR,
                &ALPHA_DESCRIPTOR,
            ],
            &[],
        );
        let names: Vec<_> = analysis
            .construction_plan()
            .unwrap()
            .steps()
            .iter()
            .map(ConstructionStep::type_name)
            .collect();

        assert_eq!(
            names,
            [
                "plan::Alpha",
                "plan::Database",
                "plan::Service",
                "plan::Zeta"
            ]
        );
    }

    #[test]
    fn satisfied_nodes_are_inspectable_but_absent_from_the_plan() {
        let satisfied = [
            SatisfiedProvider::provided::<Alpha>(),
            SatisfiedProvider::preconstructed::<PlanDatabase>(),
        ];
        let analysis = analyze_parts(&[&ALPHA_DESCRIPTOR, &PLAN_DATABASE_DESCRIPTOR], &satisfied);

        assert!(analysis.construction_plan().unwrap().steps().is_empty());
        assert_eq!(
            analysis.graph().provider::<Alpha>().unwrap().state(),
            ProviderState::Provided
        );
        assert_eq!(
            analysis.graph().provider::<PlanDatabase>().unwrap().state(),
            ProviderState::Preconstructed
        );
    }
}

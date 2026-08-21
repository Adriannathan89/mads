//! Deterministic validation of provider descriptors and dependency edges.

#![allow(dead_code)]

use std::cmp::Ordering;

use crate::{
    Diagnostic, MADS001, MADS002, MADS003, ProviderDescriptor, ProviderOrigin, ProviderState,
    ProviderVisibility, SourceLocation,
};

use super::model::{
    ApplicationGraph, ConstructionPlan, DependencyEdge, GraphAnalysis, ProviderNode,
    SatisfiedProvider,
};

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

    diagnostics.sort_by(PendingDiagnostic::order);
    let diagnostics = diagnostics
        .into_iter()
        .map(|pending| pending.diagnostic)
        .collect::<Vec<_>>();
    let construction_plan = diagnostics
        .is_empty()
        .then(|| ConstructionPlan { steps: Vec::new() });

    GraphAnalysis {
        graph: ApplicationGraph {
            providers,
            dependencies,
        },
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
        ConstructionContext, DependencyDescriptor, Diagnostic, ErasedProvider, MADS001, MADS002,
        MADS003, ProviderDescriptor, ProviderFuture, ProviderKind, ProviderOrigin, ProviderState,
        ProviderVisibility, SourceLocation,
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
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use std::any::TypeId;
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use super::analyze_parts;
    use crate::auto_configuration::{
        AutoConfigurationContext, AutoConfigurationContribution, AutoConfigurationDescriptor,
        AutoConfigurationEvaluation, AutoConfigurationInputs, AutoConfigurationReasonCode,
        AutoConfigurationReport, AutoConfigurationStatus,
    };
    use crate::graph::SatisfiedProvider;
    use crate::{
        Config, DependencyDescriptor, Diagnostic, Error, MADS007, ProviderDescriptor,
        ProviderFuture, ProviderKind, ProviderVisibility, SourceLocation,
    };

    struct DefaultResource;
    struct CascadingResource;
    struct ResourceConsumer;
    struct SecondResourceConsumer;

    static EVALUATIONS: Mutex<Vec<&str>> = Mutex::new(Vec::new());
    static EVALUATION_COUNT: AtomicUsize = AtomicUsize::new(0);
    static CONFIG_READS: AtomicUsize = AtomicUsize::new(0);

    static DEFAULT_DEPENDENCY: [DependencyDescriptor; 1] = [DependencyDescriptor::new(
        "DefaultResource",
        default_resource_type_id,
    )];
    static REQUIRING_PROVIDERS: [&ProviderDescriptor; 1] = [&RESOURCE_CONSUMER];
    static TWO_REQUIRING_PROVIDERS: [&ProviderDescriptor; 2] =
        [&RESOURCE_CONSUMER, &SECOND_RESOURCE_CONSUMER];
    static FIRST_REQUIRING_PROVIDER: [&ProviderDescriptor; 1] = [&RESOURCE_CONSUMER];

    static RESOURCE_CONSUMER: ProviderDescriptor = ProviderDescriptor::new(
        ProviderKind::Provider,
        "ResourceConsumer",
        resource_consumer_type_id,
        &DEFAULT_DEPENDENCY,
        ProviderVisibility::Public,
        SourceLocation::new("analysis.rs", 101, 1),
        unused_constructor,
    );
    static SECOND_RESOURCE_CONSUMER: ProviderDescriptor = ProviderDescriptor::new(
        ProviderKind::Provider,
        "SecondResourceConsumer",
        second_resource_consumer_type_id,
        &DEFAULT_DEPENDENCY,
        ProviderVisibility::Public,
        SourceLocation::new("analysis.rs", 102, 1),
        unused_constructor,
    );

    static ZETA_DESCRIPTOR: AutoConfigurationDescriptor = AutoConfigurationDescriptor::new(
        "zeta",
        "DefaultResource",
        default_resource_type_id,
        SourceLocation::new("analysis.rs", 201, 1),
        evaluate_zeta,
        unused_applier,
    );
    static ALPHA_DESCRIPTOR: AutoConfigurationDescriptor = AutoConfigurationDescriptor::new(
        "alpha",
        "CascadingResource",
        cascading_resource_type_id,
        SourceLocation::new("analysis.rs", 202, 1),
        evaluate_alpha,
        unused_applier,
    );
    static DUPLICATE_FIRST: AutoConfigurationDescriptor = AutoConfigurationDescriptor::new(
        "duplicate",
        "DefaultResource",
        default_resource_type_id,
        SourceLocation::new("analysis.rs", 203, 1),
        evaluate_counted,
        unused_applier,
    );
    static DUPLICATE_SECOND: AutoConfigurationDescriptor = AutoConfigurationDescriptor::new(
        "duplicate",
        "CascadingResource",
        cascading_resource_type_id,
        SourceLocation::new("analysis.rs", 204, 1),
        evaluate_counted,
        unused_applier,
    );
    static ACTIVE_FIRST: AutoConfigurationDescriptor = AutoConfigurationDescriptor::new(
        "active.first",
        "DefaultResource",
        default_resource_type_id,
        SourceLocation::new("analysis.rs", 205, 1),
        evaluate_active_default,
        unused_applier,
    );
    static ACTIVE_SECOND: AutoConfigurationDescriptor = AutoConfigurationDescriptor::new(
        "active.second",
        "DefaultResource",
        default_resource_type_id,
        SourceLocation::new("analysis.rs", 206, 1),
        evaluate_active_default,
        unused_applier,
    );
    static CONDITIONAL_DESCRIPTOR: AutoConfigurationDescriptor = AutoConfigurationDescriptor::new(
        "conditional",
        "DefaultResource",
        default_resource_type_id,
        SourceLocation::new("analysis.rs", 207, 1),
        evaluate_conditional,
        unused_applier,
    );
    static FAILING_DESCRIPTOR: AutoConfigurationDescriptor = AutoConfigurationDescriptor::new(
        "failing",
        "DefaultResource",
        default_resource_type_id,
        SourceLocation::new("analysis.rs", 208, 1),
        evaluate_failure,
        unused_applier,
    );
    static FIRST_DESCRIPTOR: AutoConfigurationDescriptor = AutoConfigurationDescriptor::new(
        "first",
        "DefaultResource",
        default_resource_type_id,
        SourceLocation::new("analysis.rs", 209, 1),
        evaluate_active_default,
        unused_applier,
    );
    static CASCADE_DESCRIPTOR: AutoConfigurationDescriptor = AutoConfigurationDescriptor::new(
        "second",
        "CascadingResource",
        cascading_resource_type_id,
        SourceLocation::new("analysis.rs", 210, 1),
        evaluate_conditional_cascade,
        unused_applier,
    );

    #[test]
    fn descriptors_are_evaluated_in_identifier_order() {
        EVALUATIONS.lock().unwrap().clear();
        let analysis = analyze_parts(
            &[&ZETA_DESCRIPTOR, &ALPHA_DESCRIPTOR],
            &[],
            &[],
            &Config::empty(),
            &AutoConfigurationInputs::default(),
        );
        assert_eq!(*EVALUATIONS.lock().unwrap(), ["alpha", "zeta"]);
        assert_eq!(
            analysis
                .reports
                .iter()
                .map(AutoConfigurationReport::identifier)
                .collect::<Vec<_>>(),
            ["alpha", "zeta"],
        );
    }

    #[test]
    fn duplicate_identifiers_fail_without_calling_evaluators() {
        EVALUATION_COUNT.store(0, Ordering::SeqCst);
        let analysis = analyze_parts(
            &[&DUPLICATE_FIRST, &DUPLICATE_SECOND],
            &[],
            &[],
            &Config::empty(),
            &AutoConfigurationInputs::default(),
        );
        assert_eq!(EVALUATION_COUNT.load(Ordering::SeqCst), 0);
        assert_eq!(analysis.diagnostics[0].code(), MADS007);
        assert_eq!(
            analysis.reports[0].status(),
            AutoConfigurationStatus::Failed
        );
        assert_eq!(
            analysis.reports[0].reason_code().as_str(),
            "duplicate_identifier"
        );
    }

    #[test]
    fn conflicting_active_defaults_fail_instead_of_using_priority() {
        let analysis = analyze_parts(
            &[&ACTIVE_FIRST, &ACTIVE_SECOND],
            &REQUIRING_PROVIDERS,
            &[],
            &Config::empty(),
            &AutoConfigurationInputs::default(),
        );
        assert_eq!(analysis.diagnostics[0].code(), MADS007);
        assert!(analysis.selected.is_empty());
        assert_eq!(analysis.covered_missing, [TypeId::of::<DefaultResource>()]);
        assert!(analysis.reports.iter().all(|report| {
            report.status() == AutoConfigurationStatus::Failed
                && report.reason_code().as_str() == "conflicting_default"
        }));
    }

    #[test]
    fn override_precedes_absent_requirement_and_configuration_access() {
        CONFIG_READS.store(0, Ordering::SeqCst);
        let satisfied = [SatisfiedProvider::provided::<DefaultResource>()];
        let analysis = analyze_parts(
            &[&CONDITIONAL_DESCRIPTOR],
            &[],
            &satisfied,
            &Config::empty(),
            &AutoConfigurationInputs::default(),
        );
        assert_eq!(
            analysis.reports[0].status(),
            AutoConfigurationStatus::Overridden
        );
        assert_eq!(analysis.reports[0].reason_code().as_str(), "user_override");
        assert_eq!(CONFIG_READS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn absent_requirement_skips_without_configuration_access() {
        CONFIG_READS.store(0, Ordering::SeqCst);
        let analysis = analyze_parts(
            &[&CONDITIONAL_DESCRIPTOR],
            &[],
            &[],
            &Config::empty(),
            &AutoConfigurationInputs::default(),
        );
        assert_eq!(
            analysis.reports[0].status(),
            AutoConfigurationStatus::Skipped
        );
        assert_eq!(
            analysis.reports[0].reason_code().as_str(),
            "requirement_absent"
        );
        assert_eq!(CONFIG_READS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn failed_condition_covers_the_missing_output_and_retains_all_consumers() {
        let analysis = analyze_parts(
            &[&FAILING_DESCRIPTOR],
            &TWO_REQUIRING_PROVIDERS,
            &[],
            &Config::empty(),
            &AutoConfigurationInputs::default(),
        );
        assert_eq!(
            analysis.reports[0].status(),
            AutoConfigurationStatus::Failed
        );
        assert_eq!(analysis.reports[0].requirements().len(), 2);
        assert_eq!(analysis.covered_missing, [TypeId::of::<DefaultResource>()]);
        assert!(analysis.virtual_satisfied.is_empty());
    }

    #[test]
    fn one_pass_does_not_turn_selected_outputs_into_new_requirements() {
        let analysis = analyze_parts(
            &[&FIRST_DESCRIPTOR, &CASCADE_DESCRIPTOR],
            &FIRST_REQUIRING_PROVIDER,
            &[],
            &Config::empty(),
            &AutoConfigurationInputs::default(),
        );
        assert_eq!(
            analysis.reports[0].status(),
            AutoConfigurationStatus::Active
        );
        assert_eq!(
            analysis.reports[1].status(),
            AutoConfigurationStatus::Skipped
        );
    }

    fn default_resource_type_id() -> TypeId {
        TypeId::of::<DefaultResource>()
    }

    fn cascading_resource_type_id() -> TypeId {
        TypeId::of::<CascadingResource>()
    }

    fn resource_consumer_type_id() -> TypeId {
        TypeId::of::<ResourceConsumer>()
    }

    fn second_resource_consumer_type_id() -> TypeId {
        TypeId::of::<SecondResourceConsumer>()
    }

    fn unused_constructor<'a>(_: &'a crate::ConstructionContext<'a>) -> ProviderFuture<'a> {
        Box::pin(async { unreachable!("analysis must not invoke provider constructors") })
    }

    fn unused_applier(
        _: &crate::auto_configuration::AutoConfigurationApplyContext<'_>,
    ) -> crate::Result<AutoConfigurationContribution> {
        unreachable!("analysis must not invoke auto-configuration appliers")
    }

    fn active(
        requirements: Vec<crate::AutoConfigurationRequirement>,
    ) -> AutoConfigurationEvaluation {
        AutoConfigurationEvaluation::active(
            AutoConfigurationReasonCode::new("conditions_matched"),
            "conditions matched",
            requirements,
            Vec::new(),
        )
    }

    fn evaluate_alpha(context: &AutoConfigurationContext<'_>) -> AutoConfigurationEvaluation {
        assert!(!context.has_provider::<CascadingResource>());
        assert!(context.requirements::<CascadingResource>().is_empty());
        EVALUATIONS.lock().unwrap().push("alpha");
        active(Vec::new())
    }

    fn evaluate_zeta(context: &AutoConfigurationContext<'_>) -> AutoConfigurationEvaluation {
        assert!(!context.has_provider::<DefaultResource>());
        assert!(context.requirements::<DefaultResource>().is_empty());
        EVALUATIONS.lock().unwrap().push("zeta");
        active(Vec::new())
    }

    fn evaluate_counted(_: &AutoConfigurationContext<'_>) -> AutoConfigurationEvaluation {
        EVALUATION_COUNT.fetch_add(1, Ordering::SeqCst);
        active(Vec::new())
    }

    fn evaluate_active_default(
        context: &AutoConfigurationContext<'_>,
    ) -> AutoConfigurationEvaluation {
        active(context.requirements::<DefaultResource>())
    }

    fn evaluate_conditional(context: &AutoConfigurationContext<'_>) -> AutoConfigurationEvaluation {
        if context.has_provider::<DefaultResource>() {
            return AutoConfigurationEvaluation::overridden(
                AutoConfigurationReasonCode::new("user_override"),
                "application provider overrides this default",
                Vec::new(),
                Vec::new(),
            );
        }

        let requirements = context.requirements::<DefaultResource>();
        if requirements.is_empty() {
            return AutoConfigurationEvaluation::skipped(
                AutoConfigurationReasonCode::new("requirement_absent"),
                "no provider requires this default",
                requirements,
                Vec::new(),
            );
        }

        CONFIG_READS.fetch_add(1, Ordering::SeqCst);
        assert!(context.config().is_empty());
        active(requirements)
    }

    fn evaluate_failure(context: &AutoConfigurationContext<'_>) -> AutoConfigurationEvaluation {
        let requirements = context.requirements::<DefaultResource>();
        AutoConfigurationEvaluation::failed(
            AutoConfigurationReasonCode::new("missing_configuration"),
            "required configuration is missing",
            requirements,
            Vec::new(),
            Error::new(Diagnostic::new(
                MADS007,
                "auto-configuration failed",
                "the fake default could not be configured",
            )),
        )
    }

    fn evaluate_conditional_cascade(
        context: &AutoConfigurationContext<'_>,
    ) -> AutoConfigurationEvaluation {
        let requirements = context.requirements::<CascadingResource>();
        if requirements.is_empty() {
            AutoConfigurationEvaluation::skipped(
                AutoConfigurationReasonCode::new("requirement_absent"),
                "no provider requires this default",
                requirements,
                Vec::new(),
            )
        } else {
            active(requirements)
        }
    }
}
// Deterministic, side-effect-free official auto-configuration evaluation.

use std::any::TypeId;
use std::collections::HashSet;

use crate::graph::SatisfiedProvider;
use crate::{
    AutoConfigurationReasonCode, AutoConfigurationReport, AutoConfigurationStatus, Config,
    Diagnostic, Error, MADS007, ProviderDescriptor,
};

use super::{
    AutoConfigurationContext, AutoConfigurationDescriptor, AutoConfigurationEvaluation,
    AutoConfigurationInputs,
};

/// Builder-facing results of one official auto-configuration evaluation pass.
pub(crate) struct AutoConfigurationAnalysis {
    pub(crate) reports: Vec<AutoConfigurationReport>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) failure: Option<Error>,
    pub(crate) virtual_satisfied: Vec<SatisfiedProvider>,
    pub(crate) covered_missing: Vec<TypeId>,
    pub(crate) selected: Vec<&'static AutoConfigurationDescriptor>,
}

pub(crate) fn analyze_parts(
    descriptors: &[&'static AutoConfigurationDescriptor],
    providers: &[&'static ProviderDescriptor],
    satisfied: &[SatisfiedProvider],
    config: &Config,
    inputs: &AutoConfigurationInputs,
) -> AutoConfigurationAnalysis {
    let mut descriptors = descriptors.to_vec();
    descriptors.sort_by(|left, right| {
        left.identifier()
            .cmp(right.identifier())
            .then_with(|| left.output_type_name().cmp(right.output_type_name()))
            .then_with(|| location_order(left.location(), right.location()))
    });

    let mut decisions = Vec::with_capacity(descriptors.len());
    let mut start = 0;
    while start < descriptors.len() {
        let identifier = descriptors[start].identifier();
        let mut end = start + 1;
        while end < descriptors.len() && descriptors[end].identifier() == identifier {
            end += 1;
        }

        if end - start > 1 {
            let diagnostic = duplicate_identifier_diagnostic(descriptors[start]);
            for (index, descriptor) in descriptors[start..end].iter().enumerate() {
                decisions.push(Decision {
                    descriptor,
                    evaluation: AutoConfigurationEvaluation::failed(
                        AutoConfigurationReasonCode::new("duplicate_identifier"),
                        "an auto-configuration identifier is duplicated",
                        Vec::new(),
                        Vec::new(),
                        Error::new(diagnostic.clone()),
                    ),
                    has_direct_requirement: has_direct_requirement(descriptor, providers),
                    emit_diagnostic: index == 0,
                });
            }
        } else {
            let descriptor = descriptors[start];
            let context = AutoConfigurationContext::new(
                descriptor.identifier(),
                config,
                providers,
                satisfied,
                inputs,
            );
            let evaluation = (descriptor.evaluator())(&context);
            decisions.push(Decision {
                descriptor,
                evaluation,
                has_direct_requirement: has_direct_requirement(descriptor, providers),
                emit_diagnostic: true,
            });
        }
        start = end;
    }

    for index in 0..decisions.len() {
        if decisions[index].evaluation.status() != AutoConfigurationStatus::Active {
            continue;
        }

        let output_type_id = decisions[index].descriptor.output_type_id();
        let conflicts = decisions
            .iter()
            .enumerate()
            .filter_map(|(candidate_index, candidate)| {
                (candidate.evaluation.status() == AutoConfigurationStatus::Active
                    && candidate.descriptor.output_type_id() == output_type_id)
                    .then_some(candidate_index)
            })
            .collect::<Vec<_>>();
        if conflicts.len() < 2 || conflicts[0] != index {
            continue;
        }

        let diagnostic = conflicting_default_diagnostic(decisions[index].descriptor);
        for (position, conflict_index) in conflicts.into_iter().enumerate() {
            let decision = &mut decisions[conflict_index];
            decision.evaluation.replace_failure(
                AutoConfigurationReasonCode::new("conflicting_default"),
                "multiple auto-configuration defaults conflict",
                Error::new(diagnostic.clone()),
            );
            decision.emit_diagnostic = position == 0;
        }
    }

    let reports = decisions
        .iter()
        .map(|decision| decision.evaluation.report(decision.descriptor))
        .collect();
    let diagnostics = decisions
        .iter()
        .filter(|decision| decision.emit_diagnostic)
        .filter_map(|decision| decision.evaluation.failure())
        .map(|error| error.diagnostic().clone())
        .collect();
    let selected = decisions
        .iter()
        .filter(|decision| decision.evaluation.status() == AutoConfigurationStatus::Active)
        .map(|decision| decision.descriptor)
        .collect::<Vec<_>>();
    let virtual_satisfied = selected
        .iter()
        .map(|descriptor| {
            SatisfiedProvider::auto_configured(
                descriptor.output_type_id(),
                descriptor.output_type_name(),
            )
        })
        .collect();
    let covered_missing = covered_missing(&decisions);
    let failure = decisions.iter_mut().find_map(|decision| {
        (decision.evaluation.status() == AutoConfigurationStatus::Failed
            && decision
                .evaluation
                .failure()
                .is_some_and(|error| std::error::Error::source(error).is_some()))
        .then(|| decision.evaluation.take_failure())
        .flatten()
    });

    AutoConfigurationAnalysis {
        reports,
        diagnostics,
        failure,
        virtual_satisfied,
        covered_missing,
        selected,
    }
}

struct Decision {
    descriptor: &'static AutoConfigurationDescriptor,
    evaluation: AutoConfigurationEvaluation,
    has_direct_requirement: bool,
    emit_diagnostic: bool,
}

fn has_direct_requirement(
    descriptor: &AutoConfigurationDescriptor,
    providers: &[&ProviderDescriptor],
) -> bool {
    let output_type_id = descriptor.output_type_id();
    providers.iter().any(|provider| {
        provider
            .dependencies()
            .iter()
            .any(|dependency| dependency.type_id() == output_type_id)
    })
}

fn covered_missing(decisions: &[Decision]) -> Vec<TypeId> {
    let mut outputs = decisions
        .iter()
        .filter(|decision| {
            decision.evaluation.status() == AutoConfigurationStatus::Failed
                && decision.has_direct_requirement
        })
        .map(|decision| {
            (
                decision.descriptor.output_type_name(),
                decision.descriptor.output_type_id(),
            )
        })
        .collect::<Vec<_>>();
    outputs.sort_by(|left, right| left.0.cmp(right.0));

    let mut seen = HashSet::new();
    outputs
        .into_iter()
        .filter_map(|(_, type_id)| seen.insert(type_id).then_some(type_id))
        .collect()
}

fn duplicate_identifier_diagnostic(descriptor: &AutoConfigurationDescriptor) -> Diagnostic {
    Diagnostic::new(
        MADS007,
        "duplicate auto-configuration identifier",
        "multiple official auto-configurations use the same stable identifier",
    )
    .with_subject(descriptor.identifier())
    .with_location(descriptor.location())
    .with_suggestion("use a unique official auto-configuration identifier")
}

fn conflicting_default_diagnostic(descriptor: &AutoConfigurationDescriptor) -> Diagnostic {
    Diagnostic::new(
        MADS007,
        "conflicting auto-configuration defaults",
        "multiple official auto-configurations selected the same output type",
    )
    .with_subject(descriptor.output_type_name())
    .with_location(descriptor.location())
    .with_suggestion("remove one conflicting official auto-configuration")
}

fn location_order(left: crate::SourceLocation, right: crate::SourceLocation) -> std::cmp::Ordering {
    left.file
        .cmp(right.file)
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.column.cmp(&right.column))
}

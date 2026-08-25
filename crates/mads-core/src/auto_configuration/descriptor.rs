//! Document-hidden contracts for official auto-configuration integrations.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use crate::graph::SatisfiedProvider;
use crate::{
    AutoConfigurationConfigEvidence, AutoConfigurationReasonCode, AutoConfigurationReport,
    AutoConfigurationRequirement, AutoConfigurationStatus, Config, ErasedProvider, Error,
    LifecycleHook, ProviderDescriptor, SourceLocation,
};

/// Evaluates the side-effect-free conditions for an official integration.
#[doc(hidden)]
pub type AutoConfigurationEvaluator =
    for<'a> fn(&AutoConfigurationContext<'a>) -> AutoConfigurationEvaluation;

/// Applies one selected official integration after combined validation succeeds.
#[doc(hidden)]
pub type AutoConfigurationApplier =
    for<'a> fn(&AutoConfigurationApplyContext<'a>) -> crate::Result<AutoConfigurationContribution>;

/// Static metadata for an official MADS auto-configuration integration.
///
/// This document-hidden contract is reserved for official integrations.
#[doc(hidden)]
pub struct AutoConfigurationDescriptor {
    identifier: &'static str,
    output_type_name: &'static str,
    output_type_id: fn() -> TypeId,
    location: SourceLocation,
    evaluator: AutoConfigurationEvaluator,
    applier: AutoConfigurationApplier,
}

impl AutoConfigurationDescriptor {
    /// Creates an official integration descriptor.
    ///
    /// This document-hidden constructor is reserved for official integrations.
    #[doc(hidden)]
    pub const fn new(
        identifier: &'static str,
        output_type_name: &'static str,
        output_type_id: fn() -> TypeId,
        location: SourceLocation,
        evaluator: AutoConfigurationEvaluator,
        applier: AutoConfigurationApplier,
    ) -> Self {
        Self {
            identifier,
            output_type_name,
            output_type_id,
            location,
            evaluator,
            applier,
        }
    }

    /// Returns the stable official integration identifier.
    #[doc(hidden)]
    pub const fn identifier(&self) -> &'static str {
        self.identifier
    }

    /// Returns the output type name used in public reports.
    #[doc(hidden)]
    pub const fn output_type_name(&self) -> &'static str {
        self.output_type_name
    }

    /// Returns the runtime identifier of the integration output type.
    #[doc(hidden)]
    pub fn output_type_id(&self) -> TypeId {
        (self.output_type_id)()
    }

    /// Returns the static source location of this integration descriptor.
    #[doc(hidden)]
    pub const fn location(&self) -> SourceLocation {
        self.location
    }

    /// Returns the side-effect-free evaluator for this descriptor.
    #[doc(hidden)]
    pub const fn evaluator(&self) -> AutoConfigurationEvaluator {
        self.evaluator
    }

    /// Returns the apply callback for a selected descriptor.
    #[doc(hidden)]
    pub const fn applier(&self) -> AutoConfigurationApplier {
        self.applier
    }
}

/// Read-only state available to an official condition evaluator.
///
/// This document-hidden contract is reserved for official integrations.
#[doc(hidden)]
pub struct AutoConfigurationContext<'a> {
    identifier: &'static str,
    config: &'a Config,
    providers: &'a [&'static ProviderDescriptor],
    satisfied: &'a [SatisfiedProvider],
    inputs: &'a AutoConfigurationInputs,
}

impl<'a> AutoConfigurationContext<'a> {
    pub(crate) const fn new(
        identifier: &'static str,
        config: &'a Config,
        providers: &'a [&'static ProviderDescriptor],
        satisfied: &'a [SatisfiedProvider],
        inputs: &'a AutoConfigurationInputs,
    ) -> Self {
        Self {
            identifier,
            config,
            providers,
            satisfied,
            inputs,
        }
    }

    /// Returns the immutable builder configuration.
    #[doc(hidden)]
    pub const fn config(&self) -> &Config {
        self.config
    }

    /// Returns whether an application-controlled provider owns `T`.
    #[doc(hidden)]
    pub fn has_provider<T>(&self) -> bool
    where
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        self.satisfied
            .iter()
            .any(|provider| provider.type_id == type_id)
            || self
                .providers
                .iter()
                .any(|provider| provider.type_id() == type_id)
    }

    /// Returns every direct catalog requirement for `T` in deterministic order.
    #[doc(hidden)]
    pub fn requirements<T>(&self) -> Vec<AutoConfigurationRequirement>
    where
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let mut requirements = self
            .providers
            .iter()
            .filter(|provider| {
                provider
                    .dependencies()
                    .iter()
                    .any(|dependency| dependency.type_id() == type_id)
            })
            .map(|provider| {
                AutoConfigurationRequirement::new(provider.type_name(), Some(provider.location()))
            })
            .collect::<Vec<_>>();
        requirements.sort_by(requirement_order);
        requirements
    }

    /// Returns the private input registered for this descriptor and type.
    #[doc(hidden)]
    pub fn input<T>(&self) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        self.inputs.get(self.identifier)
    }
}

/// Read-only state available while applying a selected official integration.
///
/// This document-hidden contract is reserved for official integrations.
#[doc(hidden)]
pub struct AutoConfigurationApplyContext<'a> {
    identifier: &'static str,
    config: &'a Config,
    inputs: &'a AutoConfigurationInputs,
}

impl<'a> AutoConfigurationApplyContext<'a> {
    pub(crate) const fn new(
        identifier: &'static str,
        config: &'a Config,
        inputs: &'a AutoConfigurationInputs,
    ) -> Self {
        Self {
            identifier,
            config,
            inputs,
        }
    }

    /// Returns the immutable builder configuration.
    #[doc(hidden)]
    pub const fn config(&self) -> &Config {
        self.config
    }

    /// Returns the private input registered for this descriptor and type.
    #[doc(hidden)]
    pub fn input<T>(&self) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        self.inputs.get(self.identifier)
    }
}

/// The result of evaluating one official auto-configuration descriptor.
///
/// This document-hidden contract is reserved for official integrations.
#[doc(hidden)]
pub struct AutoConfigurationEvaluation {
    status: AutoConfigurationStatus,
    reason_code: AutoConfigurationReasonCode,
    explanation: String,
    requirements: Vec<AutoConfigurationRequirement>,
    configuration: Vec<AutoConfigurationConfigEvidence>,
    failure: Option<Error>,
}

impl AutoConfigurationEvaluation {
    /// Records a selected default whose conditions matched.
    #[doc(hidden)]
    pub fn active(
        reason_code: AutoConfigurationReasonCode,
        explanation: impl Into<String>,
        requirements: Vec<AutoConfigurationRequirement>,
        configuration: Vec<AutoConfigurationConfigEvidence>,
    ) -> Self {
        Self::new(
            AutoConfigurationStatus::Active,
            reason_code,
            explanation,
            requirements,
            configuration,
            None,
        )
    }

    /// Records a descriptor whose conditions did not apply.
    #[doc(hidden)]
    pub fn skipped(
        reason_code: AutoConfigurationReasonCode,
        explanation: impl Into<String>,
        requirements: Vec<AutoConfigurationRequirement>,
        configuration: Vec<AutoConfigurationConfigEvidence>,
    ) -> Self {
        Self::new(
            AutoConfigurationStatus::Skipped,
            reason_code,
            explanation,
            requirements,
            configuration,
            None,
        )
    }

    /// Records a descriptor superseded by an application-controlled provider.
    #[doc(hidden)]
    pub fn overridden(
        reason_code: AutoConfigurationReasonCode,
        explanation: impl Into<String>,
        requirements: Vec<AutoConfigurationRequirement>,
        configuration: Vec<AutoConfigurationConfigEvidence>,
    ) -> Self {
        Self::new(
            AutoConfigurationStatus::Overridden,
            reason_code,
            explanation,
            requirements,
            configuration,
            None,
        )
    }

    /// Records a failed descriptor while preserving its structured error.
    #[doc(hidden)]
    pub fn failed(
        reason_code: AutoConfigurationReasonCode,
        explanation: impl Into<String>,
        requirements: Vec<AutoConfigurationRequirement>,
        configuration: Vec<AutoConfigurationConfigEvidence>,
        error: Error,
    ) -> Self {
        Self::new(
            AutoConfigurationStatus::Failed,
            reason_code,
            explanation,
            requirements,
            configuration,
            Some(error),
        )
    }

    fn new(
        status: AutoConfigurationStatus,
        reason_code: AutoConfigurationReasonCode,
        explanation: impl Into<String>,
        requirements: Vec<AutoConfigurationRequirement>,
        configuration: Vec<AutoConfigurationConfigEvidence>,
        failure: Option<Error>,
    ) -> Self {
        Self {
            status,
            reason_code,
            explanation: explanation.into(),
            requirements,
            configuration,
            failure,
        }
    }

    pub(crate) fn report(
        &self,
        descriptor: &AutoConfigurationDescriptor,
    ) -> AutoConfigurationReport {
        AutoConfigurationReport::new(
            descriptor.identifier(),
            descriptor.output_type_name(),
            self.status,
            self.reason_code,
            &self.explanation,
            self.requirements.clone(),
            self.configuration.clone(),
        )
    }

    pub(crate) const fn status(&self) -> AutoConfigurationStatus {
        self.status
    }

    pub(crate) fn failure(&self) -> Option<&Error> {
        self.failure.as_ref()
    }

    pub(crate) fn take_failure(&mut self) -> Option<Error> {
        self.failure.take()
    }

    pub(crate) fn replace_failure(
        &mut self,
        reason_code: AutoConfigurationReasonCode,
        explanation: &'static str,
        error: Error,
    ) {
        self.status = AutoConfigurationStatus::Failed;
        self.reason_code = reason_code;
        self.explanation = explanation.to_owned();
        self.failure = Some(error);
    }
}

/// The provider value and lifecycle hooks contributed by a selected descriptor.
///
/// This document-hidden contract is reserved for official integrations.
#[doc(hidden)]
pub struct AutoConfigurationContribution {
    provider: ErasedProvider,
    hooks: Vec<Box<dyn LifecycleHook>>,
}

impl AutoConfigurationContribution {
    /// Wraps an official integration's provider value.
    #[doc(hidden)]
    pub fn new<T>(value: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        Self {
            provider: Arc::new(value),
            hooks: Vec::new(),
        }
    }

    /// Adds a lifecycle hook owned by this official integration.
    #[doc(hidden)]
    pub fn with_lifecycle_hook<H>(mut self, hook: H) -> Self
    where
        H: LifecycleHook + 'static,
    {
        self.hooks.push(Box::new(hook));
        self
    }

    pub(crate) fn into_parts(self) -> (ErasedProvider, Vec<Box<dyn LifecycleHook>>) {
        (self.provider, self.hooks)
    }
}

#[derive(Default)]
pub(crate) struct AutoConfigurationInputs {
    values: HashMap<(&'static str, TypeId), Box<dyn Any + Send + Sync>>,
}

impl AutoConfigurationInputs {
    pub(crate) fn insert<T>(&mut self, identifier: &'static str, value: T) -> bool
    where
        T: Send + Sync + 'static,
    {
        let key = (identifier, TypeId::of::<T>());
        if self.values.contains_key(&key) {
            return false;
        }
        self.values.insert(key, Box::new(value));
        true
    }

    pub(crate) fn get<T>(&self, identifier: &'static str) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        self.values
            .get(&(identifier, TypeId::of::<T>()))
            .and_then(|value| value.downcast_ref())
    }
}

fn requirement_order(
    left: &AutoConfigurationRequirement,
    right: &AutoConfigurationRequirement,
) -> std::cmp::Ordering {
    left.provider_type_name()
        .cmp(right.provider_type_name())
        .then_with(|| location_order(left.location(), right.location()))
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
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

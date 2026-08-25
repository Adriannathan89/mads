//! End-to-end coverage for official auto-configuration integration.

use std::any::TypeId;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use mads_core::{AutoConfigurationReasonCode, AutoConfigurationStatus, ProviderState};
use mads_core::{
    ConfigBuilder, ConstructionContext, DependencyDescriptor, Diagnostic, ErasedProvider, Error,
    MADS003, MADS020, Mads, MapSource, ProviderDescriptor, ProviderFuture, ProviderKind,
    ProviderOrigin, ProviderVisibility, SourceLocation,
};
use tokio::sync::{Mutex, MutexGuard};

struct DefaultResource;
struct ResourceConsumer;

#[derive(Debug)]
struct FakeConfigurationCause;

impl std::fmt::Display for FakeConfigurationCause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the fake default configuration is invalid")
    }
}

impl std::error::Error for FakeConfigurationCause {}

static EVALUATIONS: AtomicUsize = AtomicUsize::new(0);
static APPLICATIONS: AtomicUsize = AtomicUsize::new(0);
static CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);
static APPLY_FAILURE: AtomicBool = AtomicBool::new(false);
static TEST_LOCK: Mutex<()> = Mutex::const_new(());

static CONSUMER_DEPENDENCIES: [DependencyDescriptor; 1] = [DependencyDescriptor::new(
    "DefaultResource",
    default_resource_type_id,
)];

fn default_resource_type_id() -> TypeId {
    TypeId::of::<DefaultResource>()
}

fn resource_consumer_type_id() -> TypeId {
    TypeId::of::<ResourceConsumer>()
}

fn resource_consumer_constructor<'a>(context: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async move {
        CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
        let _ = context.resolve::<DefaultResource>()?;
        Ok(Arc::new(ResourceConsumer) as ErasedProvider)
    })
}

fn evaluate(
    context: &mads_core::__private::AutoConfigurationContext<'_>,
) -> mads_core::__private::AutoConfigurationEvaluation {
    EVALUATIONS.fetch_add(1, Ordering::SeqCst);
    let requirements = context.requirements::<DefaultResource>();

    if context.config().get("fake.fail") == Some("true") {
        return mads_core::__private::AutoConfigurationEvaluation::failed(
            AutoConfigurationReasonCode::new("invalid_configuration"),
            "the fake default was configured to fail",
            requirements,
            Vec::new(),
            Error::with_source(
                Diagnostic::new(MADS020, "fake default failed", "fixture failure"),
                FakeConfigurationCause,
            ),
        );
    }
    if context.has_provider::<DefaultResource>() {
        return mads_core::__private::AutoConfigurationEvaluation::overridden(
            AutoConfigurationReasonCode::new("user_override"),
            "an application provider overrides the fake default",
            requirements,
            Vec::new(),
        );
    }
    if requirements.is_empty() {
        return mads_core::__private::AutoConfigurationEvaluation::skipped(
            AutoConfigurationReasonCode::new("requirement_absent"),
            "no provider requires the fake default",
            requirements,
            Vec::new(),
        );
    }

    mads_core::__private::AutoConfigurationEvaluation::active(
        AutoConfigurationReasonCode::new("conditions_matched"),
        "the fake default conditions matched",
        requirements,
        Vec::new(),
    )
}

fn apply(
    _: &mads_core::__private::AutoConfigurationApplyContext<'_>,
) -> mads_core::Result<mads_core::__private::AutoConfigurationContribution> {
    APPLICATIONS.fetch_add(1, Ordering::SeqCst);
    if APPLY_FAILURE.load(Ordering::SeqCst) {
        return Err(Error::new(Diagnostic::new(
            MADS020,
            "fake default application failed",
            "fixture failure",
        )));
    }

    Ok(mads_core::__private::AutoConfigurationContribution::new(
        DefaultResource,
    ))
}

mads_core::__private::inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Provider,
        "auto_configuration_builder::ResourceConsumer",
        resource_consumer_type_id,
        &CONSUMER_DEPENDENCIES,
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        resource_consumer_constructor,
    )
}

mads_core::__private::inventory::submit! {
    mads_core::__private::AutoConfigurationDescriptor::new(
        "fake.default",
        "auto_configuration_builder::DefaultResource",
        default_resource_type_id,
        SourceLocation::new(file!(), line!(), column!()),
        evaluate,
        apply,
    )
}

fn reset_counts() {
    EVALUATIONS.store(0, Ordering::SeqCst);
    APPLICATIONS.store(0, Ordering::SeqCst);
    CONSTRUCTIONS.store(0, Ordering::SeqCst);
    APPLY_FAILURE.store(false, Ordering::SeqCst);
}

fn test_guard() -> MutexGuard<'static, ()> {
    TEST_LOCK.blocking_lock()
}

#[test]
fn analysis_selects_without_applying_or_constructing() {
    let _guard = test_guard();
    reset_counts();
    let builder = Mads::builder();
    let first = builder.analyze();
    let second = builder.analyze();

    assert!(first.is_valid());
    assert!(second.is_valid());
    assert_eq!(EVALUATIONS.load(Ordering::SeqCst), 2);
    assert_eq!(APPLICATIONS.load(Ordering::SeqCst), 0);
    assert_eq!(CONSTRUCTIONS.load(Ordering::SeqCst), 0);
    assert_eq!(
        first.auto_configurations()[0].status(),
        AutoConfigurationStatus::Active
    );
    let node = first.graph().provider::<DefaultResource>().unwrap();
    assert_eq!(node.origin(), ProviderOrigin::AutoConfiguration);
    assert_eq!(node.state(), ProviderState::AutoConfigured);
}

#[tokio::test]
async fn build_applies_default_before_constructing_consumers_and_retains_report() {
    let _guard = TEST_LOCK.lock().await;
    reset_counts();
    let application = Mads::builder().build().await.unwrap();

    assert_eq!(APPLICATIONS.load(Ordering::SeqCst), 1);
    assert_eq!(CONSTRUCTIONS.load(Ordering::SeqCst), 1);
    assert!(application.context().resolve::<DefaultResource>().is_ok());
    assert!(application.context().resolve::<ResourceConsumer>().is_ok());
    assert_eq!(
        application.auto_configurations()[0].status(),
        AutoConfigurationStatus::Active
    );
    assert_eq!(
        application
            .graph()
            .provider::<DefaultResource>()
            .unwrap()
            .origin(),
        ProviderOrigin::AutoConfiguration,
    );
}

#[test]
fn failed_default_suppresses_only_its_redundant_missing_provider() {
    let _guard = test_guard();
    reset_counts();
    let config = ConfigBuilder::new()
        .source(MapSource::new("test", [("fake.fail", "true")]))
        .build()
        .unwrap();
    let analysis = Mads::builder_with_config(config).analyze();

    assert!(!analysis.is_valid());
    assert_eq!(
        analysis.auto_configurations()[0].status(),
        AutoConfigurationStatus::Failed
    );
    assert_eq!(
        analysis
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == MADS003)
            .count(),
        0
    );
    assert_eq!(APPLICATIONS.load(Ordering::SeqCst), 0);
    assert_eq!(CONSTRUCTIONS.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_default_build_preserves_the_evaluator_error_source() {
    let _guard = TEST_LOCK.lock().await;
    reset_counts();
    let config = ConfigBuilder::new()
        .source(MapSource::new("test", [("fake.fail", "true")]))
        .build()
        .unwrap();

    let Err(error) = Mads::builder_with_config(config).build().await else {
        panic!("the failed fake default must reject the build");
    };

    assert_eq!(error.code(), MADS020);
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(APPLICATIONS.load(Ordering::SeqCst), 0);
    assert_eq!(CONSTRUCTIONS.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn provided_value_overrides_the_default() {
    let _guard = TEST_LOCK.lock().await;
    reset_counts();
    let mut builder = Mads::builder();
    builder.provide(DefaultResource).unwrap();
    let application = builder.build().await.unwrap();

    assert_eq!(APPLICATIONS.load(Ordering::SeqCst), 0);
    assert_eq!(
        application.auto_configurations()[0].status(),
        AutoConfigurationStatus::Overridden
    );
    assert_eq!(
        application
            .graph()
            .provider::<DefaultResource>()
            .unwrap()
            .origin(),
        ProviderOrigin::Provided,
    );
}

#[tokio::test]
async fn apply_failure_prevents_every_provider_constructor() {
    let _guard = TEST_LOCK.lock().await;
    reset_counts();
    APPLY_FAILURE.store(true, Ordering::SeqCst);
    let Err(error) = Mads::builder().build().await else {
        panic!("the fake default application must fail the build");
    };
    APPLY_FAILURE.store(false, Ordering::SeqCst);

    assert_eq!(error.code(), MADS020);
    assert_eq!(APPLICATIONS.load(Ordering::SeqCst), 1);
    assert_eq!(CONSTRUCTIONS.load(Ordering::SeqCst), 0);
}

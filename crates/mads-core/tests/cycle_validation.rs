//! Integration test proving cycle validation precedes construction.

use std::any::TypeId;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use mads_core::{
    ConstructionContext, DependencyDescriptor, ErasedProvider, MADS005, Mads, ProviderDescriptor,
    ProviderFuture, ProviderKind, ProviderVisibility, SourceLocation,
};

static CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);

struct CycleAlpha;
struct CycleBeta;

fn alpha_type_id() -> TypeId {
    TypeId::of::<CycleAlpha>()
}

fn beta_type_id() -> TypeId {
    TypeId::of::<CycleBeta>()
}

fn alpha_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async {
        CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
        Ok(Arc::new(CycleAlpha) as ErasedProvider)
    })
}

fn beta_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async {
        CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
        Ok(Arc::new(CycleBeta) as ErasedProvider)
    })
}

static ALPHA_DEPENDENCIES: [DependencyDescriptor; 1] = [DependencyDescriptor::new(
    "cycle_validation::CycleBeta",
    beta_type_id,
)];

static BETA_DEPENDENCIES: [DependencyDescriptor; 1] = [DependencyDescriptor::new(
    "cycle_validation::CycleAlpha",
    alpha_type_id,
)];

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Service,
        "cycle_validation::CycleAlpha",
        alpha_type_id,
        &ALPHA_DEPENDENCIES,
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        alpha_constructor,
    )
}

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Repository,
        "cycle_validation::CycleBeta",
        beta_type_id,
        &BETA_DEPENDENCIES,
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        beta_constructor,
    )
}

#[tokio::test]
async fn multi_node_cycle_reports_mads005_before_any_constructor_runs() {
    CONSTRUCTIONS.store(0, Ordering::SeqCst);

    let Err(error) = Mads::builder().build().await else {
        panic!("the dependency cycle must reject the graph");
    };

    assert_eq!(error.code(), MADS005);
    assert_eq!(CONSTRUCTIONS.load(Ordering::SeqCst), 0);
}

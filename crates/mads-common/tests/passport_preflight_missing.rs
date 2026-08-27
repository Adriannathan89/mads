//! Guards require a matching managed Passport strategy.

#![cfg(all(feature = "http", feature = "jwt"))]

use std::sync::atomic::{AtomicUsize, Ordering};

use mads_common::{
    MADS130, PassportPrincipal,
    core::{LifecycleFuture, LifecycleHook, Mads},
};

struct Principal;

impl PassportPrincipal for Principal {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

#[mads_common::routes]
#[mads_common::guard(strategy = "missing", principal = Principal)]
#[allow(dead_code)]
trait ProtectedRoutes {
    #[mads_common::get("/")]
    async fn profile(&self);
}

static ORDINARY_CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);
static LIFECYCLE_STARTS: AtomicUsize = AtomicUsize::new(0);

struct OrdinaryProvider;

#[mads_core::provider]
fn ordinary_provider() -> OrdinaryProvider {
    ORDINARY_CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
    OrdinaryProvider
}

struct CountingHook;

impl LifecycleHook for CountingHook {
    fn name(&self) -> &str {
        "missing-preflight-counter"
    }

    fn start<'a>(&'a self, _: &'a mads_core::ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            LIFECYCLE_STARTS.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }

    fn stop<'a>(&'a self, _: &'a mads_core::ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn missing_strategy_fails_before_construction_or_lifecycle() {
    ORDINARY_CONSTRUCTIONS.store(0, Ordering::SeqCst);
    LIFECYCLE_STARTS.store(0, Ordering::SeqCst);
    let mut builder = Mads::builder();
    builder.lifecycle_hook(CountingHook);

    let analysis = builder.analyze();
    assert_eq!(analysis.diagnostics()[0].code(), MADS130);
    assert!(
        analysis.diagnostics()[0]
            .to_string()
            .contains("missing_strategy")
    );

    let error = match builder.build().await {
        Ok(_) => panic!("missing strategies must fail before construction"),
        Err(error) => error,
    };
    assert_eq!(error.code(), MADS130);
    assert_eq!(ORDINARY_CONSTRUCTIONS.load(Ordering::SeqCst), 0);
    assert_eq!(LIFECYCLE_STARTS.load(Ordering::SeqCst), 0);
}

//! Integration test for complete-catalog construction.

use std::sync::atomic::{AtomicUsize, Ordering};

use mads::core::Mads;

static CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);

struct OtherwiseUnused;

#[mads::provider]
fn otherwise_unused() -> OtherwiseUnused {
    CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
    OtherwiseUnused
}

#[tokio::test]
async fn automatic_build_constructs_an_unreferenced_catalog_provider() {
    CONSTRUCTIONS.store(0, Ordering::SeqCst);
    let application = Mads::builder()
        .build()
        .await
        .expect("the complete catalog should build");

    assert_eq!(CONSTRUCTIONS.load(Ordering::SeqCst), 1);
    assert!(application.context().resolve::<OtherwiseUnused>().is_ok());
}

//! Integration tests for the optional Tokio runtime bootstrap.

#![cfg(feature = "runtime-tokio")]

#[test]
fn tokio_runtime_runs_a_future_to_completion() {
    let result = mads_core::runtime::block_on(async { 42 });

    assert_eq!(result, 42);
}

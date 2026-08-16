//! Runtime bootstrap helpers enabled by runtime integration features.

/// Runs a future to completion on a newly constructed multi-thread Tokio runtime.
///
/// # Panics
///
/// Panics with a stable message when the Tokio runtime cannot be constructed.
pub fn block_on<F>(future: F) -> F::Output
where
    F: std::future::Future,
{
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|_| panic!("MADS failed to construct the Tokio runtime"));
    runtime.block_on(future)
}

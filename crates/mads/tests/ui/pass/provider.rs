//! Confirms synchronous and asynchronous provider functions compile.

use mads::core::{Config, Result};

struct AsyncDirect;

struct SyncFallible;

struct CoreFallible;

#[mads::provider]
fn sync_value() -> String {
    "value".to_owned()
}

#[mads::provider]
async fn async_value(config: Config) -> mads::core::Result<usize> {
    Ok(config.len())
}

#[mads::provider]
async fn async_direct() -> AsyncDirect {
    AsyncDirect
}

#[mads::provider]
fn sync_fallible() -> Result<SyncFallible> {
    Ok(SyncFallible)
}

#[mads::provider]
fn core_fallible() -> mads_core::Result<CoreFallible> {
    Ok(CoreFallible)
}

fn __mads_construct_sync_value() {}

fn main() {
    let _user_function: fn() = __mads_construct_sync_value;
}

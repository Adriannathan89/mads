//! Public MADS.rs facade and feature composition boundary.
//!
//! The facade enables standard integrations and the Tokio runtime by default;
//! consumers can opt out of those defaults for a narrower dependency surface.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

/// Re-exports the framework-neutral MADS.rs core.
pub use mads_core as core;
pub use mads_core::{module, repository, service};

/// Re-exports standard integrations when the `common` feature is enabled.
#[cfg(feature = "common")]
pub use mads_common as common;

/// Re-exports extensions when the `extra` feature is enabled.
#[cfg(feature = "extra")]
pub use mads_extra as extra;

/// Collects ergonomic public imports as MADS.rs APIs are introduced.
pub mod prelude {
    pub use mads_core::{module, repository, service};
}

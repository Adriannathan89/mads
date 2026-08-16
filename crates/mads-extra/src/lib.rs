//! Post-v1 capability boundary for MADS.rs.
//!
//! Scheduled extension APIs are not implemented in v0.1.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

/// Exposes the framework-neutral core boundary to future extensions.
pub use mads_core as core;

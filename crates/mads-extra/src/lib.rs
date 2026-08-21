//! Post-v1 capability boundary for MADS.rs.
//!
//! The `mads foundation` command reports this boundary as reserved because its
//! scheduled extension APIs are not implemented in v0.2.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

/// Exposes the framework-neutral core boundary to future extensions.
pub use mads_core as core;

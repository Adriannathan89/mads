//! Standard backend integration boundary for MADS.rs.
//!
//! Scheduled HTTP and database APIs are not implemented in v0.1.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

/// Exposes the framework-neutral core boundary to future integrations.
pub use mads_core as core;

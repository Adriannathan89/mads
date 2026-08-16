//! Framework-neutral runtime contracts for MADS.rs.
//!
//! The v0.1 foundation reserves this crate for runtime semantics and re-exports
//! its procedural macros, without introducing HTTP or database integrations.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

/// Re-exports the core procedural macros when they are implemented.
#[allow(unused_imports)]
pub use mads_core_macros::*;

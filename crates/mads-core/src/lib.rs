//! Framework-neutral runtime contracts for MADS.rs.
//!
//! The v0.1 foundation reserves this crate for runtime semantics and re-exports
//! its procedural macros, without introducing HTTP or database integrations.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod config;
mod context;
mod diagnostic;
mod registry;

pub use config::{Config, ConfigBuilder, ConfigSource, ConfigValue, EnvSource, MapSource};
pub use context::{ApplicationContext, ConstructionContext};
pub use diagnostic::{
    Diagnostic, DiagnosticCode, Error, MADS001, MADS003, MADS004, MADS010, MADS011, MADS020,
    Result, SourceLocation,
};
pub use registry::{ErasedProvider, ProviderRegistry};

/// Re-exports the core procedural macros when they are implemented.
#[allow(unused_imports)]
pub use mads_core_macros::*;

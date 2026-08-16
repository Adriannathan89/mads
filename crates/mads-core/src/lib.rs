//! Framework-neutral runtime contracts for MADS.rs.
//!
//! The v0.1 foundation reserves this crate for runtime semantics and re-exports
//! its procedural macros, without introducing HTTP or database integrations.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod builder;
mod catalog;
mod config;
mod context;
mod descriptor;
mod diagnostic;
mod lifecycle;
mod registry;
#[cfg(feature = "runtime-tokio")]
pub mod runtime;

pub use builder::{Mads, MadsBuilder};
pub use catalog::Catalog;
pub use config::{Config, ConfigBuilder, ConfigSource, ConfigValue, EnvSource, MapSource};
pub use context::{ApplicationContext, ConstructionContext};
pub use descriptor::{
    DependencyDescriptor, ModuleDescriptor, ProviderConstructor, ProviderDescriptor,
    ProviderFuture, ProviderKind,
};
pub use diagnostic::{
    Diagnostic, DiagnosticCode, Error, MADS001, MADS003, MADS004, MADS010, MADS011, MADS020,
    Result, SourceLocation,
};
pub use lifecycle::{LifecycleFuture, LifecycleHook, LifecycleManager, LifecycleState};
pub use registry::{ErasedProvider, ProviderRegistry};

pub use mads_core_macros::{main, module, provider, repository, service};

/// Implementation details used by MADS.rs procedural macro expansions.
#[doc(hidden)]
pub mod __private {
    pub use inventory;
}

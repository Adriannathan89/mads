//! Framework-neutral runtime contracts for MADS.rs.
//!
//! MADS core provides framework-neutral application construction, lifecycle
//! management, deterministic scalar TOML configuration, optional dotenv
//! interpolation, programmatic and environment sources, provider metadata, and
//! dependency graph analysis. Dotenv values only participate in interpolation:
//! they do not mutate process state, and real process variables take
//! precedence. It re-exports the core procedural macros without introducing
//! integration dependencies.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod builder;
mod catalog;
mod config;
mod context;
mod descriptor;
mod diagnostic;
mod graph;
mod lifecycle;
mod registry;
#[cfg(feature = "runtime-tokio")]
pub mod runtime;

pub use builder::{Mads, MadsBuilder};
pub use catalog::Catalog;
pub use config::{
    Config, ConfigBuilder, ConfigSource, ConfigValue, DotenvSource, EnvSource, MapSource,
    TomlSource,
};
pub use context::{ApplicationContext, ConstructionContext};
pub use descriptor::{
    DependencyDescriptor, ModuleDescriptor, ProviderConstructor, ProviderDescriptor,
    ProviderFuture, ProviderKind, ProviderVisibility,
};
pub use diagnostic::{
    Diagnostic, DiagnosticCode, Error, MADS001, MADS002, MADS003, MADS004, MADS005, MADS006,
    MADS010, MADS011, MADS020, MADS030, Result, SourceLocation,
};
pub use graph::{
    ApplicationGraph, ConstructionPlan, ConstructionStep, DependencyEdge, GraphAnalysis,
    ProviderNode, ProviderOrigin, ProviderState,
};
pub use lifecycle::{LifecycleFuture, LifecycleHook, LifecycleManager, LifecycleState};
pub use registry::{ErasedProvider, ProviderRegistry};

pub use mads_core_macros::{main, module, provider, repository, service};

/// Implementation details used by MADS.rs procedural macro expansions.
#[doc(hidden)]
pub mod __private {
    pub use inventory;
}

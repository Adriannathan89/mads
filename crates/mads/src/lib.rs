//! Public MADS.rs facade and feature composition boundary.
//!
//! The facade enables standard integrations and the Tokio runtime by default;
//! consumers can opt out of those defaults for a narrower dependency surface.

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![doc = include_str!("../../../README.md")]

/// Re-exports the framework-neutral MADS.rs core.
pub use mads_core as core;

/// Re-exports the asynchronous MADS.rs entry-point attribute.
pub use mads_core::main;

/// Re-exports the application-module declaration attribute.
pub use mads_core::module;

/// Re-exports the general-purpose provider declaration attribute.
pub use mads_core::provider;

/// Re-exports the repository declaration attribute.
pub use mads_core::repository;

/// Re-exports the service declaration attribute.
pub use mads_core::service;

/// Re-exports standard integrations when the `common` feature is enabled.
#[cfg(feature = "common")]
pub use mads_common as common;

/// Re-exports Axum for native HTTP runtime integration.
#[cfg(feature = "common")]
pub use mads_common::axum;

/// Re-exports HTTP request extractors and their typed-header support.
#[cfg(feature = "common")]
pub use mads_common::{Header, Json, Path, Query, Request, headers};

/// Re-exports standard HTTP response types.
#[cfg(feature = "common")]
pub use mads_common::{Created, HttpError, HttpResult, NoContent};

/// Re-exports HTTP router construction and runtime startup functions.
#[cfg(feature = "common")]
pub use mads_common::{HttpRuntimeError, build_router, serve};

/// Re-exports the managed-controller declaration attribute.
#[cfg(feature = "common")]
pub use mads_common::controller;

/// Re-exports the DELETE route-contract attribute.
#[cfg(feature = "common")]
pub use mads_common::delete;

/// Re-exports the GET route-contract attribute.
#[cfg(feature = "common")]
pub use mads_common::get;

/// Re-exports the PATCH route-contract attribute.
#[cfg(feature = "common")]
pub use mads_common::patch;

/// Re-exports the POST route-contract attribute.
#[cfg(feature = "common")]
pub use mads_common::post;

/// Re-exports the route-trait declaration attribute.
#[cfg(feature = "common")]
pub use mads_common::routes;

/// Re-exports the PUT route-contract attribute.
#[cfg(feature = "common")]
pub use mads_common::put;

/// Re-exports extensions when the `extra` feature is enabled.
#[cfg(feature = "extra")]
pub use mads_extra as extra;

/// Collects application-facing MADS.rs imports.
pub mod prelude {
    /// Re-exports the asynchronous MADS.rs entry-point attribute.
    pub use mads_core::main;

    /// Re-exports the application-module declaration attribute.
    pub use mads_core::module;

    /// Re-exports the general-purpose provider declaration attribute.
    pub use mads_core::provider;

    /// Re-exports the repository declaration attribute.
    pub use mads_core::repository;

    /// Re-exports the service declaration attribute.
    pub use mads_core::service;

    /// Re-exports the managed-controller declaration attribute.
    #[cfg(feature = "common")]
    pub use mads_common::controller;

    /// Re-exports route-contract attributes.
    #[cfg(feature = "common")]
    pub use mads_common::{delete, get, patch, post, put, routes};

    /// Re-exports standard HTTP request extractors and typed-header support.
    #[cfg(feature = "common")]
    pub use mads_common::{Header, Json, Path, Query, Request, headers};

    /// Re-exports standard HTTP response types.
    #[cfg(feature = "common")]
    pub use mads_common::{Created, HttpError, HttpResult, NoContent};

    /// Re-exports HTTP router construction and runtime startup functions.
    #[cfg(feature = "common")]
    pub use mads_common::{HttpRuntimeError, build_router, serve};

    /// Re-exports types used to build, run, and inspect an application.
    pub use mads_core::{
        ApplicationContext, ApplicationGraph, Catalog, Config, ConfigBuilder, ConstructionPlan,
        ConstructionStep, DependencyEdge, Diagnostic, Error, GraphAnalysis, LifecycleHook,
        LifecycleState, Mads, MadsBuilder, ProviderNode, ProviderOrigin, ProviderState,
        ProviderVisibility, SourceLocation,
    };
}

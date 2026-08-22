//! Public MADS.rs facade and feature composition boundary.
//!
//! The facade enables the v0.4 common HTTP integration, explicit Diesel
//! persistence, and Tokio runtime by default; consumers can opt out of those
//! defaults for a narrower core-only dependency surface. Database provisioning
//! remains explicit in v0.4; v0.5 will own automatic provisioning.
//!
//! A controller implements a typed route contract. MADS validates the complete
//! route catalog, resolves the application-scoped controller once, and builds
//! an Axum router without requiring application state or a manual route list:
//!
//! ```
//! use mads::prelude::*;
//!
//! #[derive(Clone, serde::Serialize)]
//! struct User {
//!     id: u64,
//! }
//!
//! #[mads::routes(prefix = "/users")]
//! trait UserRoutes {
//!     #[mads::get("/:id")]
//!     async fn get_user(&self, id: Path<u64>) -> HttpResult<Json<User>>;
//! }
//!
//! #[mads::controller(routes = [UserRoutes])]
//! struct UserController;
//!
//! impl UserRoutes for UserController {
//!     async fn get_user(&self, Path(id): Path<u64>) -> HttpResult<Json<User>> {
//!         Ok(Json(User { id }))
//!     }
//! }
//!
//! #[mads::main]
//! async fn main() {
//!     let application = Mads::builder().build().await.unwrap();
//!     let _router = build_router(&application).unwrap();
//! }
//! ```
//!
//! Run an application with [`serve`]. Validation and router construction occur
//! before lifecycle startup or listener binding:
//!
//! ```no_run
//! use mads::prelude::*;
//!
//! #[mads::main]
//! async fn main() {
//!     let application = Mads::builder().build().await.unwrap();
//!     serve(application, "127.0.0.1:3000").await.unwrap();
//! }
//! ```
//!
//! Configure persistence explicitly before building the application:
//!
//! ```no_run
//! use mads::prelude::*;
//!
//! #[mads::main]
//! async fn main() {
//!     let config = ConfigBuilder::new()
//!         .source(mads::core::MapSource::new(
//!             "application",
//!             [("database.url", "postgres://localhost/mads")],
//!         ))
//!         .build()
//!         .unwrap();
//!     let database_config = DatabaseConfig::from_config(&config).unwrap();
//!     let mut builder = Mads::builder_with_config(config);
//!     builder
//!         .database(DatabaseBootstrap::new(database_config))
//!         .unwrap();
//!     let _application = builder.build().await.unwrap();
//! }
//! ```

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

/// Re-exports Diesel for native persistence integration.
#[cfg(feature = "common")]
pub use mads_common::diesel;

/// Re-exports Diesel migrations for native persistence integration.
#[cfg(feature = "common")]
pub use mads_common::diesel_migrations;

/// Re-exports database configuration, runtime, migration, and error contracts.
#[cfg(feature = "common")]
pub use mads_common::{
    Database, DatabaseBootstrap, DatabaseConfig, DatabaseError, DatabaseErrorKind,
    DatabasePoolStatus, DatabaseResult, MADS100, MadsBuilderDatabaseExt, MigrationReport,
    MigrationStatus,
};

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

    /// Re-exports application-facing database configuration and runtime types.
    #[cfg(feature = "common")]
    pub use mads_common::{
        Database, DatabaseBootstrap, DatabaseConfig, DatabaseError, DatabaseErrorKind,
        DatabasePoolStatus, DatabaseResult, MadsBuilderDatabaseExt, MigrationReport,
        MigrationStatus,
    };

    /// Re-exports types used to build, run, and inspect an application.
    pub use mads_core::{
        ApplicationContext, ApplicationGraph, Catalog, Config, ConfigBuilder, ConstructionPlan,
        ConstructionStep, DependencyEdge, Diagnostic, Error, GraphAnalysis, LifecycleHook,
        LifecycleState, Mads, MadsBuilder, ProviderNode, ProviderOrigin, ProviderState,
        ProviderVisibility, SourceLocation,
    };
}

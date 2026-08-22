//! Standard integration contracts for MADS.rs.
//!
//! Version 0.3 provides compile-time controller and route contracts together
//! with the Axum HTTP runtime. [`build_router`] validates every registered
//! controller before it resolves a controller or invokes a typed registrar;
//! [`serve`] performs that same validation before lifecycle startup or socket
//! binding.
//!
//! Use [`routes`] to declare an abstract route contract and [`controller`] to
//! bind one or more such contracts to a managed controller. The resulting
//! descriptors can be inspected through [`RouteCatalog`] before the HTTP
//! runtime installs handlers. [`axum`] is deliberately re-exported for native
//! extractors, response types, routers, middleware, and Tower composition.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod extract;
mod response;
mod route;
mod router;
mod server;

/// Database configuration and normalized persistence errors.
pub mod database;

/// Re-exports Axum for native runtime integration.
pub use axum;

/// Re-exports Diesel for native persistence integration.
pub use diesel;

/// Re-exports Diesel migrations for native persistence integration.
pub use diesel_migrations;

/// Database configuration, managed persistence, and normalized errors.
pub use database::{
    Database, DatabaseBootstrap, DatabaseConfig, DatabaseError, DatabaseErrorKind,
    DatabasePoolStatus, DatabaseResult, MADS100, MadsBuilderDatabaseExt, MigrationReport,
    MigrationStatus,
};

/// Standard Axum-compatible HTTP request extractors.
pub use extract::{Header, Json, Path, Query, Request, headers};

/// Standard Axum-compatible HTTP response types.
pub use response::{Created, HttpError, HttpResult, NoContent};

/// Builds an Axum router from the application's validated controllers.
pub use router::build_router;

/// Runs a validated application on the Axum HTTP runtime.
pub use server::{HttpRuntimeError, serve};

/// Exposes the framework-neutral core boundary to future integrations.
pub use mads_core as core;

/// Declares a managed controller and its route-trait contracts.
pub use mads_common_macros::controller;

/// Declares and validates a route-contract trait.
pub use mads_common_macros::routes;

/// Marks a DELETE route inside a route-contract trait.
pub use mads_common_macros::delete;

/// Marks a GET route inside a route-contract trait.
pub use mads_common_macros::get;

/// Marks a PATCH route inside a route-contract trait.
pub use mads_common_macros::patch;

/// Marks a POST route inside a route-contract trait.
pub use mads_common_macros::post;

/// Marks a PUT route inside a route-contract trait.
pub use mads_common_macros::put;

/// Static route and controller metadata types used by the contract catalog.
pub use route::{
    ControllerRegistrar, ControllerRouteDescriptor, HttpMethod, RouteCatalog,
    RouteContractDescriptor, RouteDescriptor,
};

/// Implementation details used by generated HTTP route adapters.
#[doc(hidden)]
pub mod __private {
    pub use axum::Router;
    pub use axum::routing::{delete, get, patch, post, put};

    pub use crate::route::{ValidatedRouteIter, validate_descriptors};
}

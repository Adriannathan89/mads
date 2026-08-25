//! Standard integration contracts for MADS.rs.
//!
//! Enable the `http`, `database`, `jwt`, or `cookies` feature to select only
//! the integration contracts an application needs. The framework-neutral core
//! boundary is always available through [`core`].
#![cfg_attr(
    feature = "http",
    doc = "\nThe `http` feature provides compile-time controller and route contracts and the Axum runtime. [`build_router`] validates every registered controller before it resolves a controller or invokes a typed registrar; [`serve`] performs that same validation before lifecycle startup or socket binding. Use [`routes`] to declare a route contract and [`controller`] to bind it to a managed controller. The resulting descriptors can be inspected through [`RouteCatalog`] before the HTTP runtime installs handlers. [`axum`] is deliberately re-exported for native extractors, response types, routers, middleware, and Tower composition."
)]
#![cfg_attr(
    feature = "database",
    doc = "\nThe `database` feature provides the official PostgreSQL/Diesel conditional default. When the complete provider catalog requires [`Database`] and the application has not provided one, this crate supplies the official default and its infrastructure lifecycle. Use [`DatabaseBootstrap`] as the explicit override when the application needs native Diesel control; a custom provider owns its complete lifecycle. [`MadsBuilderDatabaseExt::database_migrations`] separately registers at most one embedded migration source for an enabled default. A managed [`Database`] runs synchronous Diesel queries through its blocking pool boundary. The [`diesel`] and [`diesel_migrations`] re-exports are deliberate native escape hatches."
)]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[cfg(feature = "http")]
mod extract;
#[cfg(feature = "http")]
mod response;
#[cfg(feature = "http")]
mod route;
#[cfg(feature = "http")]
mod router;
#[cfg(feature = "http")]
mod server;

/// Database configuration and normalized persistence errors.
#[cfg(feature = "database")]
pub mod database;

/// Re-exports Axum for native runtime integration.
#[cfg(feature = "http")]
pub use axum;

/// Re-exports Diesel for native persistence integration.
#[cfg(feature = "database")]
pub use diesel;

/// Re-exports Diesel migrations for native persistence integration.
#[cfg(feature = "database")]
pub use diesel_migrations;

/// Database configuration, managed persistence, and normalized errors.
#[cfg(feature = "database")]
pub use database::{
    Database, DatabaseBootstrap, DatabaseConfig, DatabaseError, DatabaseErrorKind,
    DatabasePoolStatus, DatabaseResult, MADS100, MADS101, MadsBuilderDatabaseExt, MigrationReport,
    MigrationStatus,
};

/// Standard Axum-compatible HTTP request extractors.
#[cfg(feature = "http")]
pub use extract::{Header, Json, Path, Query, Request, headers};

/// Standard Axum-compatible HTTP response types.
#[cfg(feature = "http")]
pub use response::{Created, HttpError, HttpResult, NoContent};

/// Builds an Axum router from the application's validated controllers.
#[cfg(feature = "http")]
pub use router::build_router;

/// Runs a validated application on the Axum HTTP runtime.
#[cfg(feature = "http")]
pub use server::{HttpRuntimeError, serve};

/// Exposes the framework-neutral core boundary to future integrations.
pub use mads_core as core;

/// Declares a managed controller and its route-trait contracts.
#[cfg(feature = "http")]
pub use mads_common_macros::controller;

/// Declares and validates a route-contract trait.
#[cfg(feature = "http")]
pub use mads_common_macros::routes;

/// Marks a DELETE route inside a route-contract trait.
#[cfg(feature = "http")]
pub use mads_common_macros::delete;

/// Marks a GET route inside a route-contract trait.
#[cfg(feature = "http")]
pub use mads_common_macros::get;

/// Marks a PATCH route inside a route-contract trait.
#[cfg(feature = "http")]
pub use mads_common_macros::patch;

/// Marks a POST route inside a route-contract trait.
#[cfg(feature = "http")]
pub use mads_common_macros::post;

/// Marks a PUT route inside a route-contract trait.
#[cfg(feature = "http")]
pub use mads_common_macros::put;

/// Static route and controller metadata types used by the contract catalog.
#[cfg(feature = "http")]
pub use route::{
    ControllerRegistrar, ControllerRouteDescriptor, HttpMethod, RouteCatalog,
    RouteContractDescriptor, RouteDescriptor,
};

/// Implementation details used by generated HTTP route adapters.
#[doc(hidden)]
#[cfg(feature = "http")]
pub mod __private {
    pub use axum::Router;
    pub use axum::routing::{delete, get, patch, post, put};

    pub use crate::route::{ValidatedRouteIter, validate_descriptors};
}

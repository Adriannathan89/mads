//! Standard integration contracts for MADS.rs.
//!
//! Version 0.3 provides compile-time controller and route contracts together
//! with Axum-compatible HTTP adapters and extractors.
//!
//! Use [`routes`] to declare an abstract route contract and [`controller`] to
//! bind one or more such contracts to a managed controller. The resulting
//! descriptors can be inspected through [`RouteCatalog`] before the HTTP
//! runtime installs handlers.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod extract;
mod route;

/// Re-exports Axum for native runtime integration.
pub use axum;

/// Standard Axum-compatible HTTP request extractors.
pub use extract::{Header, Json, Path, Query, Request, headers};

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
    ControllerRouteDescriptor, HttpMethod, RouteCatalog, RouteContractDescriptor, RouteDescriptor,
};

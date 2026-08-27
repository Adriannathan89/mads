//! Public MADS.rs facade and feature composition boundary.
//!
//! The facade enables the v0.5 common HTTP integration, conditional Diesel
//! database provisioning, and Tokio runtime by default; consumers can opt out
//! of those defaults for a narrower core-only dependency surface. Database
//! configuration remains explicit: when a linked provider requires
//! the managed database service and no application provider supplies one, MADS configures the
//! default database provider from the builder's resolved [`core::Config`].
//! Applications must construct that configuration explicitly: [`core::Mads::builder`]
//! does not load configuration files, dotenv values, or environment variables.
//! The public [`AutoConfigurationReport`] records retained by analysis and a
//! built application expose redacted decision evidence only. The v0.5 engine
//! uses the complete provider catalog; module-scoped reachability is a v0.6
//! boundary.
//!
//! `DatabaseBootstrap` remains the explicit database override. Otherwise an
//! enabled database default may use one separately registered embedded migration
//! source through `MadsBuilderDatabaseExt::database_migrations`; MADS neither
//! generates migrations nor auto-loads them. HTTP listener addresses remain
//! explicit, because HTTP auto-binding and CORS are deferred to v0.5.6.
//! v0.5.5 adds opt-in cookies and Passport/JWT without login, refresh storage,
//! password hashing, CSRF, remote JWKS, JWE, or module scoping.
//!
//! A controller implements a typed route contract. MADS validates the complete
//! route catalog, resolves the application-scoped controller once, and builds
//! an Axum router without requiring application state or a manual route list:
//!
//! ```no_run
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
//! Run an application with `serve`. Validation and router construction occur
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
//! Configure the database explicitly while leaving provider provisioning to
//! the conditional default:
//!
//! ```no_run
//! use mads::{
//!     core::{ConfigBuilder, MapSource},
//!     prelude::*,
//! };
//!
//! #[mads::main]
//! async fn main() {
//!     let config = ConfigBuilder::new()
//!         .source(MapSource::new(
//!             "application",
//!             [("database.url", "postgres://localhost/mads")],
//!         ))
//!         .build()
//!         .unwrap();
//!     let _application = Mads::builder_with_config(config).build().await.unwrap();
//! }
//! ```
//!
//! `DatabaseBootstrap` remains the explicit native Diesel escape hatch. It
//! overrides the conditional default and retains direct control of the
//! database lifecycle:
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
//!
//! Register custom access and refresh strategies as managed providers. MADS
//! verifies JWT cryptography, registered claims, and token kind before either
//! strategy receives typed claims:
//!
//! ```
//! use mads::prelude::*;
//!
//! #[derive(serde::Deserialize)]
//! struct UserClaims { user_id: u64 }
//! struct UserPrincipal(u64);
//! impl PassportPrincipal for UserPrincipal {
//!     fn has_role(&self, role: &str) -> bool { role == "user" }
//!     fn has_permission(&self, permission: &str) -> bool {
//!         permission == "profile:read"
//!     }
//! }
//!
//! #[service]
//! struct AccessStrategy;
//! #[passport_strategy(name = "jwt")]
//! impl PassportStrategy for AccessStrategy {
//!     type Claims = UserClaims;
//!     type Principal = UserPrincipal;
//!     const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;
//!     async fn validate(
//!         &self,
//!         _context: &PassportContext<'_>,
//!         claims: &JwtClaims<Self::Claims>,
//!     ) -> PassportResult<Self::Principal> {
//!         Ok(UserPrincipal(claims.custom.user_id))
//!     }
//! }
//!
//! #[service]
//! struct RefreshStrategy;
//! #[passport_strategy(name = "jwt-refresh")]
//! impl PassportStrategy for RefreshStrategy {
//!     type Claims = UserClaims;
//!     type Principal = UserPrincipal;
//!     const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Refresh;
//!     async fn validate(
//!         &self,
//!         _context: &PassportContext<'_>,
//!         claims: &JwtClaims<Self::Claims>,
//!     ) -> PassportResult<Self::Principal> {
//!         Ok(UserPrincipal(claims.custom.user_id))
//!     }
//! }
//! # fn main() {}
//! ```
//!
//! Route guards inherit field by field. Method clauses replace only supplied
//! fields, cookie sources select exactly one named cookie, and `skip` removes
//! an inherited guard:
//!
//! ```
//! use mads::prelude::*;
//!
//! struct UserPrincipal;
//! impl PassportPrincipal for UserPrincipal {
//!     fn has_role(&self, role: &str) -> bool { role == "user" }
//!     fn has_permission(&self, permission: &str) -> bool {
//!         permission == "profile:read"
//!     }
//! }
//! fn owns_profile(_: &UserPrincipal) -> bool { true }
//!
//! #[routes(prefix = "/users")]
//! #[guard(
//!     strategy = "jwt",
//!     principal = UserPrincipal,
//!     source = bearer,
//!     roles(any = ["user", "admin"]),
//! )]
//! trait UserRoutes {
//!     #[get("/profile")]
//!     #[guard(
//!         permissions(all = ["profile:read"]),
//!         predicate = owns_profile,
//!     )]
//!     async fn profile(&self, principal: Authenticated<UserPrincipal>);
//!
//!     #[post("/refresh")]
//!     #[guard(strategy = "jwt-refresh", source = cookie("refresh_token"))]
//!     async fn refresh(&self);
//!
//!     #[post("/login")]
//!     #[guard(skip)]
//!     async fn login(&self);
//! }
//! # fn main() {}
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

/// Re-exports auto-configuration inspection records.
pub use mads_core::{
    AutoConfigurationConfigEvidence, AutoConfigurationReasonCode, AutoConfigurationReport,
    AutoConfigurationRequirement, AutoConfigurationStatus,
};

/// Re-exports the repository declaration attribute.
pub use mads_core::repository;

/// Re-exports the service declaration attribute.
pub use mads_core::service;

/// Re-exports enabled standard integrations.
#[cfg(any(
    feature = "http",
    feature = "database",
    feature = "jwt",
    feature = "cookies"
))]
pub use mads_common as common;

/// Re-exports Axum for native HTTP runtime integration.
#[cfg(feature = "http")]
pub use mads_common::axum;

/// Re-exports Diesel for native persistence integration.
#[cfg(feature = "database")]
pub use mads_common::diesel;

/// Re-exports Diesel migrations for native persistence integration.
#[cfg(feature = "database")]
pub use mads_common::diesel_migrations;

/// Re-exports strict cookie integration and the established cookie time types.
#[cfg(feature = "cookies")]
pub use mads_common::cookie;

/// Re-exports database configuration, runtime, migration, and error contracts.
#[cfg(feature = "database")]
pub use mads_common::{
    Database, DatabaseBootstrap, DatabaseConfig, DatabaseError, DatabaseErrorKind,
    DatabasePoolStatus, DatabaseResult, MADS100, MADS101, MadsBuilderDatabaseExt, MigrationReport,
    MigrationStatus,
};

/// Re-exports typed JWT claims, service, options, errors, and diagnostics.
#[cfg(feature = "jwt")]
pub use mads_common::{
    JwtAlgorithm, JwtClaims, JwtError, JwtErrorKind, JwtHeader, JwtResult, JwtService,
    JwtSignOptions, JwtTokenKind, JwtValidation, MADS120, MADS121, PassportConfig,
    RegisteredJwtClaims, VerifiedJwt,
};

/// Re-exports strict cookie extraction, response composition, and diagnostics.
#[cfg(feature = "cookies")]
pub use mads_common::{
    Cookie, CookieError, CookieErrorKind, CookieJar, CookieRejection, CookieResult, Expiration,
    MADS110, SameSite,
};

/// Re-exports HTTP request extractors and their typed-header support.
#[cfg(feature = "http")]
pub use mads_common::{Header, Json, Path, Query, Request, headers};

/// Re-exports standard HTTP response types.
#[cfg(feature = "http")]
pub use mads_common::{Created, HttpError, HttpResult, NoContent};

/// Re-exports HTTP router construction and runtime startup functions.
#[cfg(feature = "http")]
pub use mads_common::{HttpRuntimeError, build_router, serve};

/// Re-exports guarded Passport authentication and policy contracts.
#[cfg(all(feature = "http", feature = "jwt"))]
pub use mads_common::{
    Authenticated, ClaimsPrincipal, MADS130, MADS131, PassportContext, PassportError,
    PassportErrorKind, PassportGuard, PassportGuardBuilder, PassportPrincipal, PassportRejection,
    PassportResult, PassportStrategy, TokenSource, VerifiedToken, guard, passport_strategy,
};

/// Re-exports the managed-controller declaration attribute.
#[cfg(feature = "http")]
pub use mads_common::controller;

/// Re-exports the DELETE route-contract attribute.
#[cfg(feature = "http")]
pub use mads_common::delete;

/// Re-exports the GET route-contract attribute.
#[cfg(feature = "http")]
pub use mads_common::get;

/// Re-exports the PATCH route-contract attribute.
#[cfg(feature = "http")]
pub use mads_common::patch;

/// Re-exports the POST route-contract attribute.
#[cfg(feature = "http")]
pub use mads_common::post;

/// Re-exports the route-trait declaration attribute.
#[cfg(feature = "http")]
pub use mads_common::routes;

/// Re-exports the PUT route-contract attribute.
#[cfg(feature = "http")]
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
    #[cfg(feature = "http")]
    pub use mads_common::controller;

    /// Re-exports route-contract attributes.
    #[cfg(feature = "http")]
    pub use mads_common::{delete, get, patch, post, put, routes};

    /// Re-exports standard HTTP request extractors and typed-header support.
    #[cfg(feature = "http")]
    pub use mads_common::{Header, Json, Path, Query, Request, headers};

    /// Re-exports standard HTTP response types.
    #[cfg(feature = "http")]
    pub use mads_common::{Created, HttpError, HttpResult, NoContent};

    /// Re-exports HTTP router construction and runtime startup functions.
    #[cfg(feature = "http")]
    pub use mads_common::{HttpRuntimeError, build_router, serve};

    /// Re-exports application-facing Passport guards, strategies, and extractors.
    #[cfg(all(feature = "http", feature = "jwt"))]
    pub use mads_common::{
        Authenticated, ClaimsPrincipal, MADS130, MADS131, PassportContext, PassportError,
        PassportErrorKind, PassportGuard, PassportGuardBuilder, PassportPrincipal,
        PassportRejection, PassportResult, PassportStrategy, TokenSource, VerifiedToken, guard,
        passport_strategy,
    };

    /// Re-exports application-facing database configuration and runtime types.
    #[cfg(feature = "database")]
    pub use mads_common::{
        Database, DatabaseBootstrap, DatabaseConfig, DatabaseError, DatabaseErrorKind,
        DatabasePoolStatus, DatabaseResult, MadsBuilderDatabaseExt, MigrationReport,
        MigrationStatus,
    };

    /// Re-exports application-facing Passport JWT contracts and services.
    #[cfg(feature = "jwt")]
    pub use mads_common::{
        JwtAlgorithm, JwtClaims, JwtError, JwtErrorKind, JwtHeader, JwtResult, JwtService,
        JwtSignOptions, JwtTokenKind, JwtValidation, MADS120, MADS121, PassportConfig,
        RegisteredJwtClaims, VerifiedJwt,
    };

    /// Re-exports strict cookie extraction, response composition, and time types.
    #[cfg(feature = "cookies")]
    pub use mads_common::{
        Cookie, CookieError, CookieErrorKind, CookieJar, CookieRejection, CookieResult, Expiration,
        MADS110, SameSite, cookie,
    };

    /// Re-exports types used to build, run, and inspect an application.
    pub use mads_core::{
        ApplicationContext, ApplicationGraph, AutoConfigurationConfigEvidence,
        AutoConfigurationReasonCode, AutoConfigurationReport, AutoConfigurationRequirement,
        AutoConfigurationStatus, Catalog, Config, ConfigBuilder, ConstructionPlan,
        ConstructionStep, DependencyEdge, Diagnostic, Error, GraphAnalysis, LifecycleHook,
        LifecycleState, Mads, MadsBuilder, ProviderNode, ProviderOrigin, ProviderState,
        ProviderVisibility, SourceLocation,
    };
}

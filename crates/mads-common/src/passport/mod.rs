//! Typed Passport principals, request context, and normalized failures.
//!
//! Passport is available only when both the `http` and `jwt` features are
//! enabled. Guards install [`Authenticated`] principals and [`VerifiedToken`]
//! values into request extensions for typed handler extraction.
//!
//! A managed access strategy receives verified claims and returns the current
//! application identity. A refresh strategy is application-defined and selects
//! the refresh token profile:
//!
//! ```ignore
//! use mads_common::{
//!     JwtClaims, JwtTokenKind, PassportContext, PassportPrincipal,
//!     PassportResult, PassportStrategy, passport_strategy,
//! };
//!
//! #[derive(Clone, serde::Deserialize)]
//! struct UserClaims { user_id: u64 }
//! struct UserPrincipal(u64);
//! impl PassportPrincipal for UserPrincipal {
//!     fn has_role(&self, role: &str) -> bool { role == "user" }
//!     fn has_permission(&self, permission: &str) -> bool {
//!         permission == "profile:read"
//!     }
//! }
//!
//! #[mads_core::service]
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
//! #[mads_core::service]
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
//! Route policy inherits field by field. A method replaces only the fields it
//! supplies, while `skip` removes an inherited guard:
//!
//! ```ignore
//! use mads_common::{Authenticated, PassportPrincipal, guard, routes};
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
//!     #[mads_common::get("/profile")]
//!     #[guard(
//!         permissions(all = ["profile:read"]),
//!         predicate = owns_profile,
//!     )]
//!     async fn profile(&self, principal: Authenticated<UserPrincipal>);
//!
//!     #[mads_common::post("/login")]
//!     #[guard(skip)]
//!     async fn login(&self);
//! }
//! # fn main() {}
//! ```
//!
//! With the `cookies` feature, use `source = cookie("refresh_token")`. One
//! guard reads exactly one source and never falls back to Bearer. Roles,
//! permissions, and every synchronous `fn(&Principal) -> bool` predicate are
//! separate AND clauses.

mod context;
mod error;
mod guard;
mod principal;
mod strategy;

pub use context::PassportContext;
#[cfg(feature = "cookies")]
pub use context::PassportCookies;
pub use error::{PassportError, PassportErrorKind, PassportRejection, PassportResult};
pub use guard::{
    BuiltinGuardAdapter, GuardCatalog, GuardDescriptor, GuardPredicate, GuardPredicateAdapter,
    NativePassportGuardService, PassportGuard, PassportGuardBuilder, PassportGuardLayer,
    PassportGuardService, PassportGuardState, PolicyClause, PolicyMode, TokenSource,
};
pub use principal::{Authenticated, ClaimsPrincipal, PassportPrincipal, VerifiedToken};
pub use strategy::{
    ErasedAuthentication, PassportStrategy, PassportStrategyAdapter, PassportStrategyBinding,
    PassportStrategyCatalog, PassportStrategyDescriptor, PassportStrategyFuture,
    PassportStrategyPreflight,
};

/// Passport strategy registration, resolution, or type mismatch.
pub const MADS130: mads_core::DiagnosticCode = mads_core::DiagnosticCode::new("MADS130");

/// Guard metadata or authentication-policy failure.
pub const MADS131: mads_core::DiagnosticCode = mads_core::DiagnosticCode::new("MADS131");

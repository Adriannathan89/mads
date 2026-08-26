//! Typed Passport principals, request context, and normalized failures.
//!
//! Passport is available only when both the `http` and `jwt` features are
//! enabled. Guards install [`Authenticated`] principals and [`VerifiedToken`]
//! values into request extensions for typed handler extraction.

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

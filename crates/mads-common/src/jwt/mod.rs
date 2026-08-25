//! Typed JSON Web Token contracts.
//!
//! This module owns the framework's closed algorithm set, access and refresh
//! token profiles, typed claim wrappers, explicit operation options, and
//! redacted errors. Cryptographic service construction is added separately.

mod claims;
mod config;
mod error;

pub use claims::{
    JwtAlgorithm, JwtClaims, JwtHeader, JwtSignOptions, JwtTokenKind, JwtValidation,
    RegisteredJwtClaims, VerifiedJwt,
};
pub use config::PassportConfig;
pub use error::{JwtError, JwtErrorKind, JwtResult};

/// JWT signing, verification, or key-operation failure.
pub const MADS120: mads_core::DiagnosticCode = mads_core::DiagnosticCode::new("MADS120");

/// JWT auto-configuration failure.
pub const MADS121: mads_core::DiagnosticCode = mads_core::DiagnosticCode::new("MADS121");

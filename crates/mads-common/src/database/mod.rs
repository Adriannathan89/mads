//! Database configuration and normalized persistence errors.

mod config;
mod error;

pub use config::DatabaseConfig;
pub use error::{DatabaseError, DatabaseErrorKind, DatabaseResult};

/// Database configuration or persistence integration failure.
pub const MADS100: mads_core::DiagnosticCode = mads_core::DiagnosticCode::new("MADS100");

//! Database configuration and normalized persistence errors.

mod config;
mod error;
mod pool;

pub use config::DatabaseConfig;
pub use error::{DatabaseError, DatabaseErrorKind, DatabaseResult};
pub use pool::{Database, DatabasePoolStatus};

/// Database configuration or persistence integration failure.
pub const MADS100: mads_core::DiagnosticCode = mads_core::DiagnosticCode::new("MADS100");

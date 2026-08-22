//! Explicit PostgreSQL database configuration, lifecycle, and persistence APIs.

mod config;
mod error;
mod lifecycle;
mod migration;
mod pool;

pub use config::DatabaseConfig;
pub use error::{DatabaseError, DatabaseErrorKind, DatabaseResult};
pub use lifecycle::{DatabaseBootstrap, MadsBuilderDatabaseExt};
pub use migration::{MigrationReport, MigrationStatus};
pub use pool::{Database, DatabasePoolStatus};

/// Database configuration or persistence integration failure.
pub const MADS100: mads_core::DiagnosticCode = mads_core::DiagnosticCode::new("MADS100");

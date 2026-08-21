//! Standard Axum-compatible HTTP request extractors.
//!
//! These are direct re-exports of Axum and axum-extra types. Their rejection,
//! ordering, body-consumption, and deserialization behavior remains defined by
//! those crates; applications can use other native extractors through
//! [`crate::axum`].

pub use axum::Json;
pub use axum::extract::{Path, Query, Request};
pub use axum_extra::TypedHeader as Header;
pub use axum_extra::headers;

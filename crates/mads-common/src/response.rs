//! HTTP response types for MADS route handlers.
//!
//! [`HttpResult`] represents delivery failures as stable HTTP responses. It is
//! intentionally separate from [`mads_core::Result`], which remains the result
//! type for framework construction and bootstrap operations. Handlers may also
//! return any native Axum [`IntoResponse`](axum::response::IntoResponse) type.

use std::{error::Error, fmt};

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

const INTERNAL_SERVER_ERROR_MESSAGE: &str = "internal server error";

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'static str,
    message: &'a str,
}

/// An HTTP error rendered as a stable JSON response.
///
/// Construct values with [`HttpError::bad_request`],
/// [`HttpError::not_found`], [`HttpError::conflict`], or
/// [`HttpError::internal`] so callers do not depend on individual variants.
#[non_exhaustive]
#[derive(Debug)]
pub enum HttpError {
    /// A request that cannot be processed because its client-facing input is invalid.
    BadRequest(String),
    /// A requested resource that could not be found.
    NotFound(String),
    /// A request that conflicts with the current state of a resource.
    Conflict(String),
    /// An unexpected server-side failure whose source is not exposed to clients.
    Internal(Box<dyn Error + Send + Sync>),
}

impl HttpError {
    /// Creates a 400 Bad Request error with a safe client-facing message.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    /// Creates a 404 Not Found error with a safe client-facing message.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    /// Creates a 409 Conflict error with a safe client-facing message.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    /// Creates a 500 Internal Server Error without exposing its source to clients.
    pub fn internal(source: impl Error + Send + Sync + 'static) -> Self {
        Self::Internal(Box::new(source))
    }

    fn response_parts(&self) -> (StatusCode, &'static str, &str) {
        match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message),
            Self::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
            Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                INTERNAL_SERVER_ERROR_MESSAGE,
            ),
        }
    }
}

impl fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.response_parts().2)
    }
}

impl Error for HttpError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Internal(source) => Some(source.as_ref()),
            Self::BadRequest(_) | Self::NotFound(_) | Self::Conflict(_) => None,
        }
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let (status, code, message) = self.response_parts();
        let mut response = Json(ErrorEnvelope {
            error: ErrorBody { code, message },
        })
        .into_response();
        *response.status_mut() = status;
        response
    }
}

/// A result type for HTTP handlers that use [`HttpError`] as their error type.
pub type HttpResult<T> = std::result::Result<T, HttpError>;

/// A response wrapper that changes a successful inner response to 201 Created.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Created<T>(pub T);

impl<T> IntoResponse for Created<T>
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut response = self.0.into_response();
        *response.status_mut() = StatusCode::CREATED;
        response
    }
}

/// An empty response with the 204 No Content status.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NoContent;

impl IntoResponse for NoContent {
    fn into_response(self) -> Response {
        StatusCode::NO_CONTENT.into_response()
    }
}

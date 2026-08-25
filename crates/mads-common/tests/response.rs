//! Public HTTP response contract tests.

#![cfg(feature = "http")]

use std::io;

use mads_common::{
    Created, HttpError, HttpResult, Json, NoContent,
    axum::{
        body::{Body, to_bytes},
        http::{
            HeaderValue, StatusCode,
            header::{CONTENT_TYPE, HeaderName},
        },
        response::{IntoResponse, Response},
    },
};

async fn assert_error_response(error: HttpError, expected_status: StatusCode, expected_body: &str) {
    let response = error.into_response();

    assert_eq!(response.status(), expected_status);
    assert_eq!(
        response.headers().get(CONTENT_TYPE),
        Some(&HeaderValue::from_static("application/json"))
    );
    assert_eq!(
        std::str::from_utf8(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("error response body must be readable"),
        )
        .expect("error response body must be UTF-8"),
        expected_body
    );
}

#[tokio::test]
async fn created_and_no_content_set_exact_statuses() {
    let created = Created(Json(serde_json::json!({"id": 7}))).into_response();
    assert_eq!(created.status(), StatusCode::CREATED);

    let empty = NoContent.into_response();
    assert_eq!(empty.status(), StatusCode::NO_CONTENT);
    assert!(
        to_bytes(empty.into_body(), usize::MAX)
            .await
            .expect("no-content response body must be readable")
            .is_empty()
    );
}

#[tokio::test]
async fn created_preserves_the_inner_response_headers_and_body() {
    let mut inner = Response::new(Body::from("created user"));
    inner.headers_mut().insert(
        HeaderName::from_static("x-request-id"),
        HeaderValue::from_static("request-123"),
    );

    let response = Created(inner).into_response();

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response.headers().get("x-request-id"),
        Some(&HeaderValue::from_static("request-123"))
    );
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("created response body must be readable"),
        "created user"
    );
}

#[tokio::test]
async fn bad_request_serializes_its_safe_message() {
    assert_error_response(
        HttpError::bad_request("email is required"),
        StatusCode::BAD_REQUEST,
        r#"{"error":{"code":"bad_request","message":"email is required"}}"#,
    )
    .await;
}

#[tokio::test]
async fn not_found_serializes_its_safe_message() {
    assert_error_response(
        HttpError::not_found("user was not found"),
        StatusCode::NOT_FOUND,
        r#"{"error":{"code":"not_found","message":"user was not found"}}"#,
    )
    .await;
}

#[tokio::test]
async fn conflict_serializes_its_safe_message() {
    assert_error_response(
        HttpError::conflict("email is already registered"),
        StatusCode::CONFLICT,
        r#"{"error":{"code":"conflict","message":"email is already registered"}}"#,
    )
    .await;
}

#[tokio::test]
async fn internal_hides_its_source_in_the_response() {
    assert_error_response(
        HttpError::internal(io::Error::other("database credentials unavailable")),
        StatusCode::INTERNAL_SERVER_ERROR,
        r#"{"error":{"code":"internal","message":"internal server error"}}"#,
    )
    .await;
}

#[test]
fn http_result_alias_uses_http_error() {
    let result: HttpResult<()> = Err(HttpError::bad_request("email is required"));

    assert!(result.is_err());
}

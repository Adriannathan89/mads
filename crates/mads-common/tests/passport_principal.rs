//! Typed Passport principal and guarded extractor contracts.

#![cfg(all(feature = "http", feature = "jwt"))]

use std::collections::BTreeSet;

use mads_common::{
    Authenticated, PassportError, PassportErrorKind, PassportPrincipal, PassportRejection,
    VerifiedToken,
    axum::{
        extract::FromRequestParts,
        http::{Request, StatusCode, header::WWW_AUTHENTICATE},
        response::IntoResponse,
    },
};

#[derive(mads_common::PassportPrincipal)]
struct UserPrincipal {
    id: u64,
    #[roles]
    roles: Vec<String>,
    #[permissions]
    permissions: BTreeSet<String>,
}

#[derive(Clone)]
struct UserClaims;

#[test]
fn derive_delegates_role_and_permission_membership() {
    let principal = UserPrincipal {
        id: 7,
        roles: vec!["user".into(), "admin".into()],
        permissions: ["profile:read".into()].into_iter().collect(),
    };

    assert!(principal.has_role("admin"));
    assert!(!principal.has_role("auditor"));
    assert!(principal.has_permission("profile:read"));
    assert!(!principal.has_permission("profile:write"));
    assert_eq!(principal.id, 7);
}

struct ManualPrincipal;

impl PassportPrincipal for ManualPrincipal {
    fn has_role(&self, role: &str) -> bool {
        role == "manual"
    }

    fn has_permission(&self, permission: &str) -> bool {
        permission == "manual:read"
    }
}

#[test]
fn principal_contract_can_be_implemented_manually() {
    let principal = ManualPrincipal;

    assert!(principal.has_role("manual"));
    assert!(principal.has_permission("manual:read"));
    assert!(!principal.has_role("derived"));
}

#[tokio::test]
async fn unguarded_authenticated_extractor_returns_typed_internal_rejection() {
    let (mut parts, ()) = Request::new(()).into_parts();

    let rejection = Authenticated::<UserPrincipal>::from_request_parts(&mut parts, &())
        .await
        .unwrap_err();

    assert_eq!(rejection.kind(), PassportErrorKind::Internal);
    assert_eq!(
        rejection.into_response().status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn unguarded_verified_token_extractor_returns_typed_internal_rejection() {
    let (mut parts, ()) = Request::new(()).into_parts();

    let rejection = VerifiedToken::<UserClaims>::from_request_parts(&mut parts, &())
        .await
        .unwrap_err();

    assert_eq!(rejection.kind(), PassportErrorKind::Internal);
    assert_eq!(
        rejection.into_response().status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn rejected_authentication_is_generic_and_advertises_bearer() {
    let response = PassportRejection::from(PassportError::reject()).into_response();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[WWW_AUTHENTICATE], "Bearer");
}

#[test]
fn internal_error_formatting_redacts_its_source() {
    let error =
        PassportError::internal(std::io::Error::other("passport-sensitive-source-sentinel"));

    assert!(
        !error
            .to_string()
            .contains("passport-sensitive-source-sentinel")
    );
    assert!(!format!("{error:?}").contains("passport-sensitive-source-sentinel"));
}

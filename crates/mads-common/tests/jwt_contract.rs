//! Public JWT value and error contracts.

use std::str::FromStr;
use std::time::Duration;

use mads_common::{
    JwtAlgorithm, JwtClaims, JwtError, JwtErrorKind, JwtHeader, JwtSignOptions, JwtTokenKind,
    JwtValidation, MADS120, MADS121, RegisteredJwtClaims, VerifiedJwt,
};

#[test]
fn algorithms_are_closed_and_case_sensitive() {
    for name in [
        "HS256", "HS384", "HS512", "RS256", "RS384", "RS512", "ES256", "ES384",
    ] {
        let algorithm = JwtAlgorithm::from_str(name).unwrap();
        assert_eq!(algorithm.as_str(), name);
    }
    assert!(JwtAlgorithm::from_str("hs256").is_err());
    assert!(JwtAlgorithm::from_str("none").is_err());
}

#[test]
fn sign_and_validation_options_always_carry_a_kind() {
    let sign = JwtSignOptions::refresh(Duration::from_secs(60))
        .subject("user-1")
        .jwt_id("refresh-1");
    assert_eq!(sign.kind(), JwtTokenKind::Refresh);
    assert_eq!(sign.lifetime(), Duration::from_secs(60));
    assert_eq!(sign.subject_value(), Some("user-1"));
    assert_eq!(sign.jwt_id_value(), Some("refresh-1"));
    assert_eq!(JwtValidation::access().kind(), JwtTokenKind::Access);
    assert_eq!(JwtValidation::refresh().kind(), JwtTokenKind::Refresh);
}

#[test]
fn errors_expose_kinds_without_exposing_context_values() {
    let error = JwtError::new(JwtErrorKind::InvalidSignature);
    assert_eq!(error.kind(), JwtErrorKind::InvalidSignature);
    assert!(!format!("{error:?}").contains("token-secret-sentinel"));
    assert_eq!(MADS120.as_str(), "MADS120");
    assert_eq!(MADS121.as_str(), "MADS121");
}

#[test]
fn claim_and_option_debug_output_is_structural_and_redacted() {
    let sentinel = "jwt-sensitive-sentinel";
    let registered = RegisteredJwtClaims {
        issuer: Some(sentinel.into()),
        subject: Some(sentinel.into()),
        audiences: vec![sentinel.into()],
        expires_at: 100,
        not_before: Some(50),
        issued_at: 40,
        jwt_id: Some(sentinel.into()),
        token_kind: JwtTokenKind::Access,
    };
    let verified = VerifiedJwt {
        header: JwtHeader {
            algorithm: JwtAlgorithm::Hs256,
            key_id: None,
            token_type: Some("mads-access+jwt".into()),
        },
        claims: JwtClaims {
            registered,
            custom: sentinel.to_owned(),
        },
    };
    let sign = JwtSignOptions::access(Duration::from_secs(60))
        .subject(sentinel)
        .jwt_id(sentinel);
    let validation = JwtValidation::access()
        .subject(sentinel)
        .issuer(sentinel)
        .audience(sentinel)
        .jwt_id(sentinel);

    assert!(!format!("{verified:?}").contains(sentinel));
    assert!(!format!("{sign:?}").contains(sentinel));
    assert!(!format!("{validation:?}").contains(sentinel));
}

//! JWT signing, verification, and untrusted decoding contracts.

#![cfg(feature = "jwt")]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use mads_common::core::{Config, ConfigBuilder, MapSource};
use mads_common::{
    JwtErrorKind, JwtService, JwtSignOptions, JwtTokenKind, JwtValidation, PassportConfig,
};
use serde::{Deserialize, Serialize};

const SECRET: &str = "01234567890123456789012345678901";

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct UserClaims {
    user_id: u64,
    roles: Vec<String>,
}

fn config<const N: usize>(values: [(&str, &str); N]) -> Config {
    ConfigBuilder::new()
        .source(MapSource::new("test", values))
        .build()
        .unwrap()
}

fn hs256_service() -> JwtService {
    let config = config([("passport.secret", SECRET)]);
    JwtService::from_passport_config(PassportConfig::from_config(&config).unwrap()).unwrap()
}

fn policy_service(issuer: &str, audiences: &[&str], clock_skew_seconds: u64) -> JwtService {
    let config = ConfigBuilder::new()
        .source(
            MapSource::new(
                "test",
                vec![
                    ("passport.secret", SECRET.to_owned()),
                    ("passport.issuer", issuer.to_owned()),
                    (
                        "passport.clock_skew_seconds",
                        clock_skew_seconds.to_string(),
                    ),
                ],
            )
            .with_string_array("passport.audiences", audiences.iter().copied()),
        )
        .build()
        .unwrap();
    JwtService::from_config(&config).unwrap()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn claims(extra: serde_json::Value) -> serde_json::Value {
    let mut object = serde_json::Map::from_iter([
        ("exp".to_owned(), serde_json::json!(now() + 60)),
        ("iat".to_owned(), serde_json::json!(now())),
        ("token_use".to_owned(), serde_json::json!("access")),
        ("user_id".to_owned(), serde_json::json!(7)),
        ("roles".to_owned(), serde_json::json!(["user"])),
    ]);
    if let serde_json::Value::Object(extra) = extra {
        object.extend(extra);
    }
    serde_json::Value::Object(object)
}

fn signed_token(header_type: &str, claims: serde_json::Value) -> String {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    header.typ = Some(header_type.to_owned());
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .unwrap()
}

fn raw_hs256_token(header_json: &str, payload_json: &str) -> String {
    let header = URL_SAFE_NO_PAD.encode(header_json);
    let payload = URL_SAFE_NO_PAD.encode(payload_json);
    let message = format!("{header}.{payload}");
    let signature = jsonwebtoken::crypto::sign(
        message.as_bytes(),
        &jsonwebtoken::EncodingKey::from_secret(SECRET.as_bytes()),
        jsonwebtoken::Algorithm::HS256,
    )
    .unwrap();
    format!("{message}.{signature}")
}

#[test]
fn service_round_trips_typed_access_claims() {
    let service = hs256_service();
    let token = service
        .sign(
            UserClaims {
                user_id: 7,
                roles: vec!["user".into()],
            },
            JwtSignOptions::access(Duration::from_secs(60))
                .subject("7")
                .jwt_id("access-7"),
        )
        .unwrap();

    let verified = service
        .verify::<UserClaims>(&token, JwtValidation::access().subject("7"))
        .unwrap();

    assert_eq!(verified.claims.custom.user_id, 7);
    assert_eq!(verified.claims.registered.token_kind, JwtTokenKind::Access);
    assert_eq!(
        verified.header.token_type.as_deref(),
        Some("mads-access+jwt")
    );
}

#[test]
fn refresh_tokens_cannot_pass_access_validation() {
    let service = hs256_service();
    let token = service
        .sign(
            UserClaims {
                user_id: 7,
                roles: vec![],
            },
            JwtSignOptions::refresh(Duration::from_secs(60)),
        )
        .unwrap();

    assert_eq!(
        service
            .verify::<UserClaims>(&token, JwtValidation::access())
            .unwrap_err()
            .kind(),
        JwtErrorKind::TokenKindMismatch,
    );
}

#[test]
fn signing_rejects_zero_lifetime_and_non_object_or_reserved_custom_claims() {
    let service = hs256_service();
    assert_eq!(
        service
            .sign(
                UserClaims {
                    user_id: 7,
                    roles: vec![],
                },
                JwtSignOptions::access(Duration::ZERO),
            )
            .unwrap_err()
            .kind(),
        JwtErrorKind::Serialization,
    );
    for custom_claims in [
        serde_json::json!("not-an-object"),
        serde_json::json!(["not-an-object"]),
        serde_json::Value::Null,
    ] {
        assert_eq!(
            service
                .sign(
                    custom_claims,
                    JwtSignOptions::access(Duration::from_secs(1))
                )
                .unwrap_err()
                .kind(),
            JwtErrorKind::Serialization,
        );
    }
    for reserved in ["iss", "sub", "aud", "exp", "nbf", "iat", "jti", "token_use"] {
        let mut custom_claims = serde_json::Map::new();
        custom_claims.insert(reserved.to_owned(), serde_json::Value::Null);
        assert_eq!(
            service
                .sign(
                    serde_json::Value::Object(custom_claims),
                    JwtSignOptions::access(Duration::from_secs(1)),
                )
                .unwrap_err()
                .kind(),
            JwtErrorKind::Serialization,
        );
    }
}

#[test]
fn oversized_input_is_rejected_before_any_decoding() {
    let service = JwtService::from_config(&config([
        ("passport.secret", "01234567890123456789012345678901"),
        ("passport.max_token_bytes", "3"),
    ]))
    .unwrap();

    assert_eq!(
        service
            .verify::<UserClaims>("this is not jwt", JwtValidation::access())
            .unwrap_err()
            .kind(),
        JwtErrorKind::TokenTooLarge,
    );
}

#[test]
fn explicit_untrusted_decoding_does_not_validate_the_signature() {
    let service = hs256_service();
    let token = service
        .sign(
            UserClaims {
                user_id: 7,
                roles: vec!["user".into()],
            },
            JwtSignOptions::access(Duration::from_secs(60)),
        )
        .unwrap();
    let mut segments: Vec<_> = token.split('.').collect();
    segments[2] = "tampered";
    let tampered = segments.join(".");

    let decoded = service.decode_unverified::<UserClaims>(&tampered).unwrap();
    assert_eq!(decoded.custom.user_id, 7);
    assert_eq!(
        service
            .verify::<UserClaims>(&tampered, JwtValidation::access())
            .unwrap_err()
            .kind(),
        JwtErrorKind::InvalidSignature,
    );
}

#[test]
fn malformed_headers_and_duplicate_claim_keys_are_rejected() {
    let service = hs256_service();
    assert_eq!(
        service.decode_header("broken").unwrap_err().kind(),
        JwtErrorKind::MalformedToken,
    );

    let duplicate_claims = raw_hs256_token(
        r#"{"alg":"HS256","typ":"mads-access+jwt"}"#,
        r#"{"exp":9999999999,"iat":1,"token_use":"access","user_id":7,"user_id":8,"roles":[]}"#,
    );
    assert_eq!(
        service
            .verify::<UserClaims>(&duplicate_claims, JwtValidation::access())
            .unwrap_err()
            .kind(),
        JwtErrorKind::MalformedToken,
    );

    for header in [
        r#"{"alg":"none","typ":"mads-access+jwt"}"#,
        r#"{"alg":"HS256","typ":"mads-access+jwt","jku":"https://attacker.invalid"}"#,
        r#"{"alg":"HS256","typ":"mads-access+jwt","x5u":"https://attacker.invalid"}"#,
    ] {
        let token = raw_hs256_token(
            header,
            r#"{"exp":9999999999,"iat":1,"token_use":"access","user_id":7,"roles":[]}"#,
        );
        assert!(service.decode_header(&token).is_err());
    }
}

#[test]
fn required_times_and_time_boundaries_are_strict() {
    let service = hs256_service();
    let missing_exp = signed_token(
        "mads-access+jwt",
        serde_json::json!({"iat": now(), "token_use": "access", "user_id": 7, "roles": []}),
    );
    assert_eq!(
        service
            .verify::<UserClaims>(&missing_exp, JwtValidation::access())
            .unwrap_err()
            .kind(),
        JwtErrorKind::MissingExpiration,
    );

    let missing_iat = signed_token(
        "mads-access+jwt",
        serde_json::json!({"exp": now() + 60, "token_use": "access", "user_id": 7, "roles": []}),
    );
    assert_eq!(
        service
            .verify::<UserClaims>(&missing_iat, JwtValidation::access())
            .unwrap_err()
            .kind(),
        JwtErrorKind::MissingIssuedAt,
    );

    let expired = signed_token(
        "mads-access+jwt",
        serde_json::json!({"exp": 0, "iat": 0, "token_use": "access", "user_id": 7, "roles": []}),
    );
    assert_eq!(
        service
            .verify::<UserClaims>(&expired, JwtValidation::access())
            .unwrap_err()
            .kind(),
        JwtErrorKind::Expired,
    );

    let future_not_before = signed_token(
        "mads-access+jwt",
        claims(serde_json::json!({"nbf": u64::MAX})),
    );
    assert_eq!(
        service
            .verify::<UserClaims>(&future_not_before, JwtValidation::access())
            .unwrap_err()
            .kind(),
        JwtErrorKind::InvalidNotBefore,
    );

    let skewed = policy_service("issuer", &["api"], 30);
    let near_skew_boundary = signed_token(
        "mads-access+jwt",
        serde_json::json!({
            "exp": now() - 29,
            "iat": now(),
            "iss": "issuer",
            "aud": ["api"],
            "token_use": "access",
            "user_id": 7,
            "roles": []
        }),
    );
    assert!(
        skewed
            .verify::<UserClaims>(&near_skew_boundary, JwtValidation::access())
            .is_ok()
    );
}

#[test]
fn issuer_audience_subject_and_jwt_id_requirements_are_enforced() {
    let service = policy_service("issuer-a", &["api-a", "api-b"], 0);
    let intersecting_audience = signed_token(
        "mads-access+jwt",
        claims(serde_json::json!({
            "iss": "issuer-a",
            "aud": ["other", "api-b"],
            "sub": "user-7",
            "jti": "token-7"
        })),
    );
    assert!(
        service
            .verify::<UserClaims>(
                &intersecting_audience,
                JwtValidation::access()
                    .subject("user-7")
                    .jwt_id("token-7")
                    .audience("api-b"),
            )
            .is_ok()
    );

    for (token, validation, expected) in [
        (
            signed_token(
                "mads-access+jwt",
                claims(serde_json::json!({"iss": "wrong", "aud": ["api-a"]})),
            ),
            JwtValidation::access(),
            JwtErrorKind::IssuerMismatch,
        ),
        (
            signed_token(
                "mads-access+jwt",
                claims(serde_json::json!({"iss": "issuer-a", "aud": ["other"]})),
            ),
            JwtValidation::access(),
            JwtErrorKind::AudienceMismatch,
        ),
        (
            signed_token(
                "mads-access+jwt",
                claims(serde_json::json!({
                    "iss": "issuer-a",
                    "aud": ["api-a"],
                    "sub": "wrong",
                    "jti": "wrong"
                })),
            ),
            JwtValidation::access().subject("expected"),
            JwtErrorKind::SubjectMismatch,
        ),
        (
            signed_token(
                "mads-access+jwt",
                claims(serde_json::json!({"iss": "issuer-a", "aud": ["api-a"]})),
            ),
            JwtValidation::access().require_subject(),
            JwtErrorKind::SubjectMismatch,
        ),
        (
            signed_token(
                "mads-access+jwt",
                claims(serde_json::json!({"iss": "issuer-a", "aud": ["api-a"]})),
            ),
            JwtValidation::access().require_jwt_id(),
            JwtErrorKind::JwtIdMismatch,
        ),
        (
            signed_token(
                "mads-access+jwt",
                claims(serde_json::json!({
                    "iss": "issuer-a",
                    "aud": ["api-a"],
                    "sub": "user",
                    "jti": "wrong"
                })),
            ),
            JwtValidation::access().jwt_id("expected"),
            JwtErrorKind::JwtIdMismatch,
        ),
    ] {
        assert_eq!(
            service
                .verify::<UserClaims>(&token, validation)
                .unwrap_err()
                .kind(),
            expected,
        );
    }
}

#[test]
fn header_type_and_token_use_must_be_a_matching_profile() {
    let service = hs256_service();
    let mismatched = signed_token("mads-refresh+jwt", claims(serde_json::json!({})));
    assert_eq!(
        service
            .verify::<UserClaims>(&mismatched, JwtValidation::access())
            .unwrap_err()
            .kind(),
        JwtErrorKind::TokenKindMismatch,
    );
    let wrong_token_use = signed_token(
        "mads-access+jwt",
        serde_json::json!({
            "exp": now() + 60,
            "iat": now(),
            "token_use": "refresh",
            "user_id": 7,
            "roles": []
        }),
    );
    assert_eq!(
        service
            .verify::<UserClaims>(&wrong_token_use, JwtValidation::access())
            .unwrap_err()
            .kind(),
        JwtErrorKind::TokenKindMismatch,
    );
}

#[test]
fn public_diagnostics_redact_tokens_and_claim_values() {
    const SENTINEL: &str = "jwt-service-sentinel-never-display";
    let config = ConfigBuilder::new()
        .source(
            MapSource::new(
                "test",
                vec![
                    (
                        "passport.secret",
                        format!("{SENTINEL}01234567890123456789012345678901"),
                    ),
                    ("passport.issuer", SENTINEL.to_owned()),
                ],
            )
            .with_string_array("passport.audiences", [SENTINEL]),
        )
        .build()
        .unwrap();
    let service = JwtService::from_config(&config).unwrap();
    let token = service
        .sign(
            UserClaims {
                user_id: 7,
                roles: vec![SENTINEL.to_owned()],
            },
            JwtSignOptions::access(Duration::from_secs(60))
                .subject(SENTINEL)
                .jwt_id(SENTINEL),
        )
        .unwrap();
    let verified = service
        .verify::<UserClaims>(&token, JwtValidation::access())
        .unwrap();
    let error = service
        .verify::<UserClaims>("not-a-token", JwtValidation::access())
        .unwrap_err();

    for output in [
        format!("{service:?}"),
        format!("{verified:?}"),
        format!("{error}"),
        format!("{error:?}"),
    ] {
        assert!(!output.contains(SENTINEL));
        assert!(!output.contains(&token));
    }
}

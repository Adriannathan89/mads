//! Passport JWT key-rotation and algorithm-confusion contracts.

#![cfg(feature = "jwt")]

use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use mads_common::core::{Config, ConfigBuilder, MapSource, TomlSource};
use mads_common::{JwtErrorKind, JwtService, JwtSignOptions, JwtValidation, PassportConfig};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

const CURRENT_PRIVATE: &[u8] = include_bytes!("fixtures/jwt/rsa-private.pem");
const CURRENT_PUBLIC: &[u8] = include_bytes!("fixtures/jwt/rsa-public.pem");
const PREVIOUS_PRIVATE: &[u8] = include_bytes!("fixtures/jwt/rsa-previous-private.pem");
const PREVIOUS_PUBLIC: &[u8] = include_bytes!("fixtures/jwt/rsa-previous-public.pem");
const HMAC_SECRET: &str = "01234567890123456789012345678901";

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct UserClaims {
    user_id: u64,
}

fn write_rotation_config(document: &str) -> (TempDir, JwtService) {
    let directory = tempfile::tempdir().unwrap();
    for (name, contents) in [
        ("current-private.pem", CURRENT_PRIVATE),
        ("current-public.pem", CURRENT_PUBLIC),
        ("previous-private.pem", PREVIOUS_PRIVATE),
        ("previous-public.pem", PREVIOUS_PUBLIC),
    ] {
        fs::write(directory.path().join(name), contents).unwrap();
    }
    let path = directory.path().join("mads.toml");
    fs::write(&path, document).unwrap();
    let config: Config = ConfigBuilder::new()
        .source(TomlSource::file(path))
        .build()
        .unwrap();
    let service =
        JwtService::from_passport_config(PassportConfig::from_config(&config).unwrap()).unwrap();
    (directory, service)
}

fn current_service() -> (TempDir, JwtService) {
    write_rotation_config(
        r#"
[passport]
active_key = "current"
algorithms = ["RS256"]

[passport.keys.current]
algorithm = "RS256"
private_key_file = "current-private.pem"
public_key_file = "current-public.pem"

[passport.keys.previous]
algorithm = "RS256"
public_key_file = "previous-public.pem"
"#,
    )
}

fn previous_service() -> (TempDir, JwtService) {
    write_rotation_config(
        r#"
[passport]
active_key = "previous"
algorithms = ["RS256"]

[passport.keys.previous]
algorithm = "RS256"
private_key_file = "previous-private.pem"
public_key_file = "previous-public.pem"
"#,
    )
}

fn hmac_service() -> JwtService {
    let config = ConfigBuilder::new()
        .source(MapSource::new("test", [("passport.secret", HMAC_SECRET)]))
        .build()
        .unwrap();
    JwtService::from_config(&config).unwrap()
}

fn access_token(service: &JwtService, user_id: u64) -> String {
    service
        .sign(
            UserClaims { user_id },
            JwtSignOptions::access(Duration::from_secs(60)),
        )
        .unwrap()
}

fn hmac_token_with_current_key_id() -> String {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    header.typ = Some("mads-access+jwt".to_owned());
    header.kid = Some("current".to_owned());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    jsonwebtoken::encode(
        &header,
        &serde_json::json!({
            "exp": now + 60,
            "iat": now,
            "token_use": "access",
            "user_id": 1,
        }),
        &jsonwebtoken::EncodingKey::from_secret(HMAC_SECRET.as_bytes()),
    )
    .unwrap()
}

fn rewrite_header(token: &str, header: &str) -> String {
    let mut parts = token.split('.');
    let _original_header = parts.next().unwrap();
    let payload = parts.next().unwrap();
    let signature = parts.next().unwrap();
    assert!(parts.next().is_none());
    format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(header),
        payload,
        signature
    )
}

fn assert_error(service: &JwtService, token: &str, expected: JwtErrorKind) {
    assert_eq!(
        service
            .verify::<UserClaims>(token, JwtValidation::access())
            .unwrap_err()
            .kind(),
        expected,
    );
}

#[test]
fn current_key_signs_while_previous_key_remains_verify_only() {
    let (_current_directory, current) = current_service();
    let (_previous_directory, previous) = previous_service();
    let current_token = access_token(&current, 1);
    let previous_token = access_token(&previous, 2);

    assert_eq!(
        current
            .decode_header(&current_token)
            .unwrap()
            .key_id
            .as_deref(),
        Some("current")
    );
    assert_eq!(
        current
            .verify::<UserClaims>(&previous_token, JwtValidation::access())
            .unwrap()
            .claims
            .custom
            .user_id,
        2,
    );
    assert_eq!(
        current
            .decode_header(&access_token(&current, 3))
            .unwrap()
            .key_id
            .as_deref(),
        Some("current")
    );
}

#[test]
fn named_keys_and_algorithms_reject_confusion_before_signature_validation() {
    let (_directory, current) = current_service();
    let current_token = access_token(&current, 1);
    let hmac_token = hmac_token_with_current_key_id();

    assert_error(
        &current,
        &rewrite_header(&current_token, r#"{"alg":"RS256","typ":"mads-access+jwt"}"#),
        JwtErrorKind::MissingKeyId,
    );
    assert_error(
        &current,
        &rewrite_header(
            &current_token,
            r#"{"alg":"RS256","kid":"unknown","typ":"mads-access+jwt"}"#,
        ),
        JwtErrorKind::UnknownKeyId,
    );
    assert_error(
        &current,
        &rewrite_header(
            &current_token,
            r#"{"alg":"RS256","kid":"previous","typ":"mads-access+jwt"}"#,
        ),
        JwtErrorKind::InvalidSignature,
    );
    assert_error(
        &current,
        &rewrite_header(
            &current_token,
            r#"{"alg":"HS256","kid":"current","typ":"mads-access+jwt"}"#,
        ),
        JwtErrorKind::AlgorithmMismatch,
    );
    assert_error(&current, &hmac_token, JwtErrorKind::AlgorithmMismatch);
    assert_error(
        &hmac_service(),
        &current_token,
        JwtErrorKind::AlgorithmMismatch,
    );
    assert_error(
        &current,
        &rewrite_header(
            &current_token,
            r#"{"alg":"none","kid":"current","typ":"mads-access+jwt"}"#,
        ),
        JwtErrorKind::DisallowedAlgorithm,
    );
    assert_error(
        &current,
        &rewrite_header(
            &current_token,
            r#"{"alg":"RS256","kid":"current","typ":"mads-access+jwt","jku":"https://attacker.invalid/keys"}"#,
        ),
        JwtErrorKind::MalformedToken,
    );
    assert_error(
        &current,
        &rewrite_header(
            &current_token,
            r#"{"alg":"RS256","kid":"current","typ":"mads-access+jwt","x5u":"https://attacker.invalid/cert"}"#,
        ),
        JwtErrorKind::MalformedToken,
    );
}

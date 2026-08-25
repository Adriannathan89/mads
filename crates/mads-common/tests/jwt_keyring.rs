//! Named Passport JWT key-ring configuration contracts.

use std::fs;

use mads_common::core::{Config, ConfigBuilder, TomlSource};
use mads_common::{JwtErrorKind, PassportConfig};
use tempfile::TempDir;

const RSA_PRIVATE: &[u8] = include_bytes!("fixtures/jwt/rsa-private.pem");
const RSA_PUBLIC: &[u8] = include_bytes!("fixtures/jwt/rsa-public.pem");
const EC256_PRIVATE: &[u8] = include_bytes!("fixtures/jwt/ec256-private.pem");
const EC256_PUBLIC: &[u8] = include_bytes!("fixtures/jwt/ec256-public.pem");
const EC384_PRIVATE: &[u8] = include_bytes!("fixtures/jwt/ec384-private.pem");
const EC384_PUBLIC: &[u8] = include_bytes!("fixtures/jwt/ec384-public.pem");

fn config_from_toml(input: &str, files: &[(&str, &[u8])]) -> (TempDir, Config) {
    let directory = tempfile::tempdir().unwrap();
    for (relative_path, contents) in files {
        let path = directory.path().join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    let path = directory.path().join("mads.toml");
    fs::write(&path, input).unwrap();
    let config = ConfigBuilder::new()
        .source(TomlSource::file(path))
        .build()
        .unwrap();
    (directory, config)
}

fn assert_invalid_configuration(input: &str, files: &[(&str, &[u8])]) {
    let (_directory, config) = config_from_toml(input, files);
    assert_eq!(
        PassportConfig::from_config(&config).unwrap_err().kind(),
        JwtErrorKind::InvalidConfiguration,
    );
}

fn assert_invalid_key_material(input: &str, files: &[(&str, &[u8])]) {
    let (_directory, config) = config_from_toml(input, files);
    assert_eq!(
        PassportConfig::from_config(&config).unwrap_err().kind(),
        JwtErrorKind::InvalidKeyMaterial,
    );
}

#[test]
fn active_and_previous_rsa_keys_resolve_from_toml_directory() {
    let (_directory, config) = config_from_toml(
        r#"
[passport]
active_key = "current"
algorithms = ["RS256"]

[passport.keys.current]
algorithm = "RS256"
private_key_file = "keys/rsa-private.pem"
public_key_file = "keys/rsa-public.pem"

[passport.keys.previous]
algorithm = "RS256"
public_key_file = "keys/previous-rsa-public.pem"
"#,
        &[
            ("keys/rsa-private.pem", RSA_PRIVATE),
            ("keys/rsa-public.pem", RSA_PUBLIC),
            ("keys/previous-rsa-public.pem", RSA_PUBLIC),
        ],
    );
    let passport = PassportConfig::from_config(&config).unwrap();

    assert_eq!(passport.active_key_id(), Some("current"));
    assert_eq!(passport.key_ids(), ["current", "previous"]);
}

#[test]
fn key_ring_graph_rules_reject_invalid_key_ids_and_selection() {
    for input in [
        r#"
[passport]
algorithms = ["HS256"]
[passport.keys.current]
algorithm = "HS256"
secret = "01234567890123456789012345678901"
"#,
        r#"
[passport]
active_key = "unknown"
algorithms = ["HS256"]
[passport.keys.current]
algorithm = "HS256"
secret = "01234567890123456789012345678901"
"#,
        r#"
[passport]
active_key = "current"
algorithms = ["HS256"]
[passport.keys.""]
algorithm = "HS256"
secret = "01234567890123456789012345678901"
"#,
        r#"
[passport]
active_key = "current"
algorithms = ["HS256"]
[passport.keys."current bear"]
algorithm = "HS256"
secret = "01234567890123456789012345678901"
"#,
        r#"
[passport]
active_key = "current"
algorithms = ["HS256"]
[passport.keys."current🐻"]
algorithm = "HS256"
secret = "01234567890123456789012345678901"
"#,
    ] {
        assert_invalid_configuration(input, &[]);
    }
}

#[test]
fn named_mode_requires_a_unique_non_empty_allowlist_and_eligible_algorithms() {
    for input in [
        r#"
[passport]
active_key = "current"
algorithms = []
[passport.keys.current]
algorithm = "HS256"
secret = "01234567890123456789012345678901"
"#,
        r#"
[passport]
active_key = "current"
algorithms = ["HS256", "HS256"]
[passport.keys.current]
algorithm = "HS256"
secret = "01234567890123456789012345678901"
"#,
        r#"
[passport]
active_key = "current"
algorithms = ["HS256"]
[passport.keys.current]
algorithm = "HS384"
secret = "012345678901234567890123456789012345678901234567"
"#,
    ] {
        assert_invalid_configuration(input, &[]);
    }
}

#[test]
fn key_material_shape_and_hmac_length_are_strict() {
    for input in [
        r#"
[passport]
active_key = "current"
algorithms = ["RS256"]
[passport.keys.current]
algorithm = "RS256"
private_key = "inline"
private_key_file = "keys/rsa-private.pem"
public_key_file = "keys/rsa-public.pem"
"#,
        r#"
[passport]
active_key = "current"
algorithms = ["RS256"]
[passport.keys.current]
algorithm = "RS256"
private_key_file = "keys/rsa-private.pem"
public_key_file = "keys/rsa-public.pem"
[passport.keys.previous]
algorithm = "RS256"
private_key_file = "keys/rsa-private.pem"
"#,
        r#"
[passport]
active_key = "current"
algorithms = ["RS256"]
[passport.keys.current]
algorithm = "RS256"
"#,
        r#"
[passport]
active_key = "current"
algorithms = ["HS256"]
[passport.keys.current]
algorithm = "HS256"
secret = "too-short"
"#,
    ] {
        assert_invalid_configuration(
            input,
            &[
                ("keys/rsa-private.pem", RSA_PRIVATE),
                ("keys/rsa-public.pem", RSA_PUBLIC),
            ],
        );
    }
}

#[test]
fn malformed_unreadable_and_wrong_curve_pem_are_invalid_key_material() {
    assert_invalid_key_material(
        r#"
[passport]
active_key = "current"
algorithms = ["RS256"]
[passport.keys.current]
algorithm = "RS256"
private_key_file = "keys/missing.pem"
public_key_file = "keys/rsa-public.pem"
"#,
        &[("keys/rsa-public.pem", RSA_PUBLIC)],
    );
    assert_invalid_key_material(
        r#"
[passport]
active_key = "current"
algorithms = ["RS256"]
[passport.keys.current]
algorithm = "RS256"
private_key_file = "keys/not-a-key.pem"
public_key_file = "keys/rsa-public.pem"
"#,
        &[
            ("keys/not-a-key.pem", b"not a pem"),
            ("keys/rsa-public.pem", RSA_PUBLIC),
        ],
    );
    assert_invalid_key_material(
        r#"
[passport]
active_key = "current"
algorithms = ["ES256"]
[passport.keys.current]
algorithm = "ES256"
private_key_file = "keys/ec384-private.pem"
public_key_file = "keys/ec384-public.pem"
"#,
        &[
            ("keys/ec384-private.pem", EC384_PRIVATE),
            ("keys/ec384-public.pem", EC384_PUBLIC),
        ],
    );
}

#[test]
fn valid_ec_key_material_is_accepted_for_the_matching_curve() {
    let (_directory, config) = config_from_toml(
        r#"
[passport]
active_key = "current"
algorithms = ["ES256"]
[passport.keys.current]
algorithm = "ES256"
private_key_file = "keys/ec256-private.pem"
public_key_file = "keys/ec256-public.pem"
"#,
        &[
            ("keys/ec256-private.pem", EC256_PRIVATE),
            ("keys/ec256-public.pem", EC256_PUBLIC),
        ],
    );

    assert!(PassportConfig::from_config(&config).is_ok());
}

#[test]
fn key_material_errors_and_debug_do_not_expose_inline_pem() {
    const SENTINEL: &str = "private-pem-sentinel-never-display";
    let (_directory, config) = config_from_toml(
        &format!(
            r#"
[passport]
active_key = "current"
algorithms = ["RS256"]
[passport.keys.current]
algorithm = "RS256"
private_key = "{SENTINEL}"
public_key_file = "keys/rsa-public.pem"
"#
        ),
        &[("keys/rsa-public.pem", RSA_PUBLIC)],
    );
    let error = PassportConfig::from_config(&config).unwrap_err();
    assert_eq!(error.kind(), JwtErrorKind::InvalidKeyMaterial);
    assert!(!format!("{error}").contains(SENTINEL));
    assert!(!format!("{error:?}").contains(SENTINEL));
}

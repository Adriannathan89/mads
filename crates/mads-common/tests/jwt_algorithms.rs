//! Passport JWT algorithm compatibility matrix.

#![cfg(feature = "jwt")]

use std::fs;
use std::time::Duration;

use mads_common::core::{Config, ConfigBuilder, TomlSource};
use mads_common::{JwtAlgorithm, JwtService, JwtSignOptions, JwtValidation, PassportConfig};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

const RSA_PRIVATE: &[u8] = include_bytes!("fixtures/jwt/rsa-private.pem");
const RSA_PUBLIC: &[u8] = include_bytes!("fixtures/jwt/rsa-public.pem");
const EC256_PRIVATE: &[u8] = include_bytes!("fixtures/jwt/ec256-private.pem");
const EC256_PUBLIC: &[u8] = include_bytes!("fixtures/jwt/ec256-public.pem");
const EC384_PRIVATE: &[u8] = include_bytes!("fixtures/jwt/ec384-private.pem");
const EC384_PUBLIC: &[u8] = include_bytes!("fixtures/jwt/ec384-public.pem");

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct UserClaims {
    user_id: u64,
}

struct AlgorithmCase {
    algorithm: JwtAlgorithm,
    secret: Option<&'static str>,
    private_key: Option<&'static [u8]>,
    public_key: Option<&'static [u8]>,
}

fn service_for(case: &AlgorithmCase) -> (TempDir, JwtService) {
    let directory = tempfile::tempdir().unwrap();
    let mut document = format!(
        "[passport]\nactive_key = \"current\"\nalgorithms = [\"{}\"]\n\n[passport.keys.current]\nalgorithm = \"{}\"\n",
        case.algorithm.as_str(),
        case.algorithm.as_str(),
    );
    if let Some(secret) = case.secret {
        document.push_str(&format!("secret = \"{secret}\"\n"));
    } else {
        fs::write(
            directory.path().join("private.pem"),
            case.private_key.unwrap(),
        )
        .unwrap();
        fs::write(
            directory.path().join("public.pem"),
            case.public_key.unwrap(),
        )
        .unwrap();
        document.push_str("private_key_file = \"private.pem\"\npublic_key_file = \"public.pem\"\n");
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

#[test]
fn every_configured_algorithm_signs_and_verifies_typed_access_claims() {
    let cases = [
        AlgorithmCase {
            algorithm: JwtAlgorithm::Hs256,
            secret: Some("01234567890123456789012345678901"),
            private_key: None,
            public_key: None,
        },
        AlgorithmCase {
            algorithm: JwtAlgorithm::Hs384,
            secret: Some("012345678901234567890123456789012345678901234567"),
            private_key: None,
            public_key: None,
        },
        AlgorithmCase {
            algorithm: JwtAlgorithm::Hs512,
            secret: Some("0123456789012345678901234567890123456789012345678901234567890123"),
            private_key: None,
            public_key: None,
        },
        AlgorithmCase {
            algorithm: JwtAlgorithm::Rs256,
            secret: None,
            private_key: Some(RSA_PRIVATE),
            public_key: Some(RSA_PUBLIC),
        },
        AlgorithmCase {
            algorithm: JwtAlgorithm::Rs384,
            secret: None,
            private_key: Some(RSA_PRIVATE),
            public_key: Some(RSA_PUBLIC),
        },
        AlgorithmCase {
            algorithm: JwtAlgorithm::Rs512,
            secret: None,
            private_key: Some(RSA_PRIVATE),
            public_key: Some(RSA_PUBLIC),
        },
        AlgorithmCase {
            algorithm: JwtAlgorithm::Es256,
            secret: None,
            private_key: Some(EC256_PRIVATE),
            public_key: Some(EC256_PUBLIC),
        },
        AlgorithmCase {
            algorithm: JwtAlgorithm::Es384,
            secret: None,
            private_key: Some(EC384_PRIVATE),
            public_key: Some(EC384_PUBLIC),
        },
    ];

    for case in &cases {
        let (_directory, service) = service_for(case);
        let token = service
            .sign(
                UserClaims { user_id: 7 },
                JwtSignOptions::access(Duration::from_secs(60)),
            )
            .unwrap();
        let header = service.decode_header(&token).unwrap();
        let verified = service
            .verify::<UserClaims>(&token, JwtValidation::access())
            .unwrap();

        assert_eq!(header.algorithm, case.algorithm);
        assert_eq!(verified.claims.custom, UserClaims { user_id: 7 });
    }
}

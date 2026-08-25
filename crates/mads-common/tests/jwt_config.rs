//! Passport JWT configuration parsing contracts.

use mads_common::core::{Config, ConfigBuilder, MapSource};
use mads_common::{JwtAlgorithm, JwtErrorKind, PassportConfig};

fn config<const N: usize>(values: [(&str, &str); N]) -> Config {
    ConfigBuilder::new()
        .source(MapSource::new("test", values))
        .build()
        .unwrap()
}

fn assert_invalid(config: &Config) {
    assert_eq!(
        PassportConfig::from_config(config).unwrap_err().kind(),
        JwtErrorKind::InvalidConfiguration,
    );
}

#[test]
fn secret_alone_means_hs256_with_strict_defaults() {
    let config = config([("passport.secret", "01234567890123456789012345678901")]);
    let passport = PassportConfig::from_config(&config).unwrap();

    assert_eq!(passport.algorithms(), &[JwtAlgorithm::Hs256]);
    assert_eq!(passport.clock_skew_seconds(), 0);
    assert_eq!(passport.max_token_bytes(), 8192);
    assert_eq!(passport.issuer(), None);
    assert!(passport.audiences().is_empty());
}

#[test]
fn each_hmac_algorithm_enforces_its_utf8_byte_minimum() {
    for (algorithm, minimum) in [("HS256", 32), ("HS384", 48), ("HS512", 64)] {
        let weak = ConfigBuilder::new()
            .source(
                MapSource::new("test", [("passport.secret", "x".repeat(minimum - 1))])
                    .with_string_array("passport.algorithms", [algorithm]),
            )
            .build()
            .unwrap();
        assert_invalid(&weak);

        let exact = ConfigBuilder::new()
            .source(
                MapSource::new("test", [("passport.secret", "x".repeat(minimum))])
                    .with_string_array("passport.algorithms", [algorithm]),
            )
            .build()
            .unwrap();
        assert_eq!(
            PassportConfig::from_config(&exact).unwrap().algorithms(),
            &[algorithm.parse().unwrap()],
        );
    }

    let utf8 = ConfigBuilder::new()
        .source(MapSource::new(
            "test",
            [("passport.secret", "🔐".repeat(8))],
        ))
        .build()
        .unwrap();
    assert_eq!(
        PassportConfig::from_config(&utf8).unwrap().algorithms(),
        &[JwtAlgorithm::Hs256],
    );
}

#[test]
fn simple_mode_rejects_incompatible_algorithm_and_named_key_settings() {
    for algorithms in [vec!["HS256", "HS384"], vec!["RS256"], vec!["ES256"]] {
        let invalid = ConfigBuilder::new()
            .source(
                MapSource::new("test", [("passport.secret", "x".repeat(64))])
                    .with_string_array("passport.algorithms", algorithms),
            )
            .build()
            .unwrap();
        assert_invalid(&invalid);
    }

    for extra in ["passport.active_key", "passport.keys.current.algorithm"] {
        let invalid = ConfigBuilder::new()
            .source(MapSource::new(
                "test",
                [
                    ("passport.secret", "x".repeat(32)),
                    (extra, "current".to_owned()),
                ],
            ))
            .build()
            .unwrap();
        assert_invalid(&invalid);
    }
}

#[test]
fn common_validation_policy_parses_and_deduplicates_in_order() {
    let config = ConfigBuilder::new()
        .source(
            MapSource::new(
                "test",
                [
                    ("passport.secret", "x".repeat(32)),
                    ("passport.issuer", "https://issuer.example".to_owned()),
                    ("passport.clock_skew_seconds", "30".to_owned()),
                    ("passport.max_token_bytes", "4096".to_owned()),
                ],
            )
            .with_string_array("passport.algorithms", ["HS256", "HS256"])
            .with_string_array("passport.audiences", ["api-b", "api-a", "api-b"]),
        )
        .build()
        .unwrap();

    let passport = PassportConfig::from_config(&config).unwrap();
    assert_eq!(passport.algorithms(), &[JwtAlgorithm::Hs256]);
    assert_eq!(passport.issuer(), Some("https://issuer.example"));
    assert_eq!(passport.audiences(), &["api-b", "api-a"]);
    assert_eq!(passport.clock_skew_seconds(), 30);
    assert_eq!(passport.max_token_bytes(), 4096);
}

#[test]
fn malformed_empty_and_wrong_shape_values_are_rejected() {
    for (key, value) in [
        ("passport.secret", ""),
        ("passport.issuer", ""),
        ("passport.clock_skew_seconds", "-1"),
        ("passport.clock_skew_seconds", "one"),
        ("passport.max_token_bytes", "0"),
        ("passport.max_token_bytes", "many"),
    ] {
        let mut values = vec![("passport.secret", "x".repeat(32))];
        values.push((key, value.to_owned()));
        let invalid = ConfigBuilder::new()
            .source(MapSource::new("test", values))
            .build()
            .unwrap();
        assert_invalid(&invalid);
    }

    for key in [
        "passport.secret",
        "passport.issuer",
        "passport.clock_skew_seconds",
        "passport.max_token_bytes",
    ] {
        let invalid = ConfigBuilder::new()
            .source(
                MapSource::new("test", [("passport.secret", "x".repeat(32))])
                    .with_string_array(key, ["wrong-shape"]),
            )
            .build()
            .unwrap();
        assert_invalid(&invalid);
    }

    for (key, values) in [
        ("passport.algorithms", Vec::<&str>::new()),
        ("passport.algorithms", vec![""]),
        ("passport.algorithms", vec!["hs256"]),
        ("passport.audiences", Vec::<&str>::new()),
        ("passport.audiences", vec![""]),
    ] {
        let invalid = ConfigBuilder::new()
            .source(
                MapSource::new("test", [("passport.secret", "x".repeat(32))])
                    .with_string_array(key, values),
            )
            .build()
            .unwrap();
        assert_invalid(&invalid);
    }

    for key in ["passport.algorithms", "passport.audiences"] {
        let invalid = config([
            ("passport.secret", "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
            (key, "wrong-shape"),
        ]);
        assert_invalid(&invalid);
    }
}

#[test]
fn debug_and_errors_never_expose_secret_values() {
    const SENTINEL: &str = "jwt-secret-sentinel-never-display";
    let valid_secret = format!("{SENTINEL}xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
    let valid = ConfigBuilder::new()
        .source(MapSource::new(
            "test",
            [("passport.secret", valid_secret.clone())],
        ))
        .build()
        .unwrap();
    let passport = PassportConfig::from_config(&valid).unwrap();
    assert!(!format!("{passport:?}").contains(SENTINEL));

    let invalid_sources = [
        MapSource::new(
            "test",
            [
                ("passport.secret", valid_secret.clone()),
                ("passport.clock_skew_seconds", SENTINEL.to_owned()),
            ],
        ),
        MapSource::new(
            "test",
            [
                ("passport.secret", valid_secret.clone()),
                ("passport.max_token_bytes", "0".to_owned()),
            ],
        ),
        MapSource::new(
            "test",
            [
                ("passport.secret", valid_secret.clone()),
                ("passport.active_key", SENTINEL.to_owned()),
            ],
        ),
        MapSource::new("test", [("passport.secret", valid_secret)])
            .with_string_array("passport.algorithms", [SENTINEL]),
    ];

    for source in invalid_sources {
        let invalid = ConfigBuilder::new().source(source).build().unwrap();
        let error = PassportConfig::from_config(&invalid).unwrap_err();
        assert!(!format!("{error}").contains(SENTINEL));
        assert!(!format!("{error:?}").contains(SENTINEL));
    }
}

//! Compile tests for direct, facade, and renamed macro expansion paths.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn attributes_expand_for_supported_dependency_paths() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = manifest_dir.join("../../target/macro-consumers");

    for consumer in [
        "facade",
        "renamed_facade",
        "core",
        "renamed_core",
        "main_facade",
        "main_renamed_facade",
        "common",
        "renamed_common",
    ] {
        let manifest = manifest_dir
            .join("tests/consumers")
            .join(consumer)
            .join("Cargo.toml");
        let output = Command::new(env!("CARGO"))
            .args(["check", "--offline", "--manifest-path"])
            .arg(&manifest)
            .env("CARGO_TARGET_DIR", &target_dir)
            .output()
            .expect("the nested Cargo check should start");

        assert!(
            output.status.success(),
            "consumer `{consumer}` failed to compile\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn conditional_routes_compile_and_register_in_both_feature_states() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = manifest_dir.join("../../target/macro-consumers");
    let manifest = manifest_dir.join("tests/consumers/conditional_common/Cargo.toml");

    for features in [None, Some("conditional-route")] {
        let mut command = Command::new(env!("CARGO"));
        command
            .args(["run", "--offline", "--manifest-path"])
            .arg(&manifest)
            .env("CARGO_TARGET_DIR", &target_dir);
        if let Some(features) = features {
            command.args(["--features", features]);
        }
        let output = command
            .output()
            .expect("the conditional consumer should start");

        assert!(
            output.status.success(),
            "conditional consumer with features {features:?} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

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

//! Process-level private inspection protocol coverage.

use std::{fs, path::Path};

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn incompatible_direct_mads_version_is_rejected_before_launch() {
    let project = tempdir().expect("temporary project should be created");
    write_project(project.path());
    let marker = project.path().join("launched");

    let mut command = Command::cargo_bin("mads").expect("CLI binary should build");
    command
        .current_dir(project.path())
        .env("INSPECTION_LAUNCH_MARKER", &marker)
        .arg("routes")
        .assert()
        .code(1)
        .stderr(contains("MADS203"))
        .stderr(contains("0.6.0-beta.1"))
        .stderr(contains("0.7"));

    assert!(!marker.exists(), "incompatible application was launched");
}

fn write_project(root: &Path) {
    fs::create_dir_all(root.join("app/src")).expect("application source directory should exist");
    fs::create_dir_all(root.join("mads/src")).expect("local MADS source directory should exist");
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"app\", \"mads\"]\ndefault-members = [\"app\"]\nresolver = \"3\"\n",
    )
    .expect("workspace manifest should be written");
    fs::write(
        root.join("mads/Cargo.toml"),
        "[package]\nname = \"mads\"\nversion = \"0.6.0-beta.1\"\nedition = \"2024\"\n",
    )
    .expect("MADS manifest should be written");
    fs::write(root.join("mads/src/lib.rs"), "").expect("MADS source should be written");
    fs::write(
        root.join("app/Cargo.toml"),
        "[package]\nname = \"inspection-version-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nmads = { path = \"../mads\" }\n",
    )
    .expect("application manifest should be written");
    fs::write(
        root.join("app/src/main.rs"),
        "fn main() { if let Ok(marker) = std::env::var(\"INSPECTION_LAUNCH_MARKER\") { std::fs::write(marker, \"launched\").unwrap(); } }\n",
    )
    .expect("application source should be written");
}

//! Process-level coverage for inspection against real MADS applications.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn one_standard_app_is_inspected_without_selectors_or_startup_effects() {
    let marker_directory = tempdir().expect("marker directory should exist");
    let marker = marker_directory.path().join("constructed");

    fixture_command("standard")
        .env("MADS_TEST_CONSTRUCTION_MARKER", &marker)
        .arg("routes")
        .assert()
        .success()
        .stdout(contains("GET"))
        .stdout(contains("/users/:id"))
        .stdout(contains("UserController"));

    assert!(!marker.exists(), "inspection constructed a provider");
}

#[test]
fn graph_and_doctor_return_deterministic_framework_evidence() {
    fixture_command("standard")
        .arg("graph")
        .assert()
        .success()
        .stdout(contains("AppModule"))
        .stdout(contains("Construction order"));

    fixture_command("standard")
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains("configuration"))
        .stdout(contains("server/CORS"));
}

#[test]
fn a_binary_without_standard_run_is_killed_and_diagnosed() {
    fixture_command("unsupported")
        .arg("doctor")
        .assert()
        .code(1)
        .stderr(contains("MADS203"))
        .stderr(contains("Mads::run::<AppModule>()"));
}

#[test]
fn invalid_graph_preserves_partial_provider_evidence() {
    fixture_command("standard")
        .args(["graph", "--bin", "invalid-graph"])
        .assert()
        .code(1)
        .stdout(contains("NeedsMissing"))
        .stderr(contains("MADS003"));
}

#[test]
fn invalid_routes_preserve_partial_route_evidence() {
    fixture_command("standard")
        .args(["routes", "--bin", "invalid-routes"])
        .assert()
        .code(1)
        .stdout(contains("GET"))
        .stdout(contains("/duplicate"))
        .stderr(contains("MADS030"));
}

fn fixture_command(name: &str) -> Command {
    let mut command = Command::cargo_bin("mads").expect("CLI binary should build");
    command.current_dir(fixture(name));
    command
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/inspection")
        .join(name)
}

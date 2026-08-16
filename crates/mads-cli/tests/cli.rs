//! Process-level tests for the MADS CLI foundation.

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn version_reports_the_workspace_version() {
    Command::cargo_bin("mads")
        .expect("binary should build")
        .arg("--version")
        .assert()
        .success()
        .stdout(contains("mads 0.1.0"));
}

#[test]
fn foundation_check_reports_available_boundaries() {
    Command::cargo_bin("mads")
        .expect("binary should build")
        .arg("foundation")
        .assert()
        .success()
        .stdout(contains("core: available"))
        .stdout(contains("common: reserved"))
        .stdout(contains("extra: reserved"));
}

#[test]
fn help_is_printed_when_no_command_is_given() {
    Command::cargo_bin("mads")
        .expect("binary should build")
        .assert()
        .success()
        .stdout(contains("Usage: mads <command>"))
        .stdout(contains("foundation"));
}

#[test]
fn unknown_arguments_are_rejected_with_help() {
    Command::cargo_bin("mads")
        .expect("binary should build")
        .args(["unknown", "extra"])
        .assert()
        .code(2)
        .stderr(contains("error: unknown argument(s): unknown extra"))
        .stderr(contains("Usage: mads <command>"));
}

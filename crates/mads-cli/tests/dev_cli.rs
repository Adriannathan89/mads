//! Black-box CLI coverage for the development command.

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn scripted_dev_command_is_advertised_by_the_cli() {
    let mut command = Command::cargo_bin("mads").unwrap();

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "dev       Watch, rebuild, and restart a MADS application",
        ));
}

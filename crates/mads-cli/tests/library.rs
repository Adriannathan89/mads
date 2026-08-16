//! Verifies that the documented CLI library target exposes the binary runner.

use mads_cli::run;

#[test]
fn library_exposes_the_cli_runner() {
    let _runner: fn() = run;
}

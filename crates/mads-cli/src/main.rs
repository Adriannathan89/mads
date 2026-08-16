//! Command-line entry point for MADS.rs development tooling.
//!
//! Command behavior and user-facing documentation are provided by `mads_cli`.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

fn main() {
    mads_cli::run();
}

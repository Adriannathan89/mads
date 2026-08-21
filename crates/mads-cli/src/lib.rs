//! Development commands for inspecting the MADS.rs v0.2 foundation.
//!
//! The `mads` executable accepts `--help` (`-h`), `--version` (`-V`), and the
//! `foundation` command. The latter reports the available core and common
//! contract boundaries without claiming future HTTP runtime support.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

/// Runs the MADS.rs CLI using the process arguments.
pub fn run() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();

    match arguments.as_slice() {
        [] => print_help(false),
        [argument] => match argument.as_str() {
            "--help" | "-h" => print_help(false),
            "--version" | "-V" => println!("mads {}", env!("CARGO_PKG_VERSION")),
            "foundation" => print_foundation(),
            _ => exit_for_unknown_arguments(&arguments),
        },
        _ => exit_for_unknown_arguments(&arguments),
    }
}

fn exit_for_unknown_arguments(arguments: &[String]) -> ! {
    eprintln!("error: unknown argument(s): {}", arguments.join(" "));
    print_help(true);
    std::process::exit(2);
}

fn print_help(to_stderr: bool) {
    let help = "Usage: mads <command>\n\nCommands:\n  foundation  Report implemented and reserved foundation boundaries\n\nOptions:\n  -h, --help     Print this help\n  -V, --version  Print the MADS.rs version";

    if to_stderr {
        eprintln!("{help}");
    } else {
        println!("{help}");
    }
}

fn print_foundation() {
    println!(
        "core: available\ncommon contracts: available\ncommon HTTP runtime: reserved\nextra: reserved"
    );
}

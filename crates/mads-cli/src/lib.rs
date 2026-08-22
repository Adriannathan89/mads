//! Development commands for inspecting MADS.rs and managing migrations.
//!
//! The `mads` executable accepts fixed help, version, foundation, and database
//! migration commands. Database commands load project configuration from the
//! current directory and return an explicit process exit code.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod command;
mod database;

use std::process::ExitCode;

use command::{Command, DatabaseCommand, ParseError};

/// Runs the MADS.rs CLI using the process arguments.
pub fn run() -> ExitCode {
    let arguments: Vec<_> = std::env::args().skip(1).collect();

    match command::parse(&arguments) {
        Ok(command) => run_command(command),
        Err(error) => {
            print_parse_error(&error);
            ExitCode::from(2)
        }
    }
}

fn run_command(command: Command) -> ExitCode {
    match command {
        Command::Help => {
            print_help(false);
            ExitCode::SUCCESS
        }
        Command::Version => {
            println!("mads {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::Foundation => {
            print_foundation();
            ExitCode::SUCCESS
        }
        Command::Database(DatabaseCommand::Help) => {
            print_database_help(false);
            ExitCode::SUCCESS
        }
        Command::Database(command) => run_database_command(command),
    }
}

fn run_database_command(command: DatabaseCommand) -> ExitCode {
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(_) => {
            eprintln!("error: could not determine project root");
            return ExitCode::from(1);
        }
    };

    match mads::core::runtime::block_on(database::execute(command, &root)) {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}

fn print_parse_error(error: &ParseError) {
    eprintln!("error: {error}");
    if error.is_database_command() {
        print_database_help(true);
    } else {
        print_help(true);
    }
}

fn print_help(to_stderr: bool) {
    let help = "Usage: mads <command>\n\nCommands:\n  foundation  Report implemented and reserved foundation boundaries\n  db          Manage Diesel migrations\n\nOptions:\n  -h, --help     Print this help\n  -V, --version  Print the MADS.rs version";

    if to_stderr {
        eprintln!("{help}");
    } else {
        println!("{help}");
    }
}

fn print_database_help(to_stderr: bool) {
    let help = "Usage: mads db <command>\n\nCommands:\n  migrate   Apply pending migrations\n  rollback  Revert the latest applied migration\n  status    Show applied and pending migrations";

    if to_stderr {
        eprintln!("{help}");
    } else {
        println!("{help}");
    }
}

fn print_foundation() {
    println!(
        "core: available\ncommon contracts: available\ncommon HTTP runtime: available\nextra: reserved"
    );
}

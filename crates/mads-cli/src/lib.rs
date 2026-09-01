//! Development commands for running MADS.rs applications and managing migrations.
//!
//! The `mads` executable exposes the v0.7 development command surface and
//! preserves application arguments supplied after `--`. CLI syntax failures
//! exit with 2; configuration, build, and operational failures exit with 1.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod command;
mod database;
#[allow(dead_code)]
mod diagnostic;
#[allow(dead_code)]
mod project;

use std::process::ExitCode;

use command::{ApplicationCommand, Command, DatabaseCommand, DatabaseInvocation, ParseError};

/// Runs the MADS.rs CLI using the process arguments.
pub fn run() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();

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
        Command::Run(command) => run_application_command(command),
        Command::Database(DatabaseInvocation {
            command: DatabaseCommand::Help,
            ..
        }) => {
            print_database_help(false);
            ExitCode::SUCCESS
        }
        Command::Database(DatabaseInvocation { command, .. }) => run_database_command(command),
    }
}

fn run_application_command(_command: ApplicationCommand) -> ExitCode {
    eprintln!("error: application execution requires Cargo target resolution");
    ExitCode::from(1)
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
    let help = "Usage: mads <command> [options]\n\nCommands:\n  run       Build and run a MADS application\n  dev       Watch, rebuild, and restart a MADS application\n  routes    Inspect application routes\n  graph     Inspect the application graph\n  doctor    Diagnose application configuration and metadata\n  db        Manage PostgreSQL migrations\n\nApplication selection:\n  -p, --package <package>\n      --bin <binary>";

    if to_stderr {
        eprintln!("{help}");
    } else {
        println!("{help}");
    }
}

fn print_database_help(to_stderr: bool) {
    let help = "Usage: mads db <command> [--package <package>]\n\nCommands:\n  generate  Generate the complete schema diff (accepts no name)\n  migrate   Apply pending migrations\n  rollback  Revert the latest applied migration\n  status    Show applied and pending migrations\n\nApplication selection:\n  -p, --package <package>";

    if to_stderr {
        eprintln!("{help}");
    } else {
        println!("{help}");
    }
}

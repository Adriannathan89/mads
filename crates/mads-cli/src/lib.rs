//! Development commands for running MADS.rs applications and managing migrations.
//!
//! The `mads` executable exposes the v0.7 development command surface and
//! preserves application arguments supplied after `--`. CLI syntax failures
//! exit with 2; configuration, build, and operational failures exit with 1.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[allow(dead_code)]
mod cargo;
mod command;
mod database;
#[allow(dead_code)]
mod diagnostic;
mod inspection;
mod process;
#[allow(dead_code)]
mod project;

use std::{ffi::OsString, io, path::PathBuf, process::ExitCode};

use command::{Command, DatabaseCommand, DatabaseInvocation, ParseError};
use diagnostic::{CliError, MADS201, MADS202};
use inspection::inspect_application;
use project::CargoProject;

/// Runs the MADS.rs CLI using the process arguments.
pub fn run() -> ExitCode {
    mads::core::runtime::block_on(run_with(
        std::env::args_os().skip(1).collect(),
        std::env::current_dir(),
    ))
}

async fn run_with(arguments: Vec<OsString>, current_dir: io::Result<PathBuf>) -> ExitCode {
    let command = match command::parse(&arguments) {
        Ok(command) => command,
        Err(error) => {
            print_parse_error(&error);
            return ExitCode::from(2);
        }
    };

    match run_command(command, current_dir).await {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

async fn run_command(
    command: Command,
    current_dir: io::Result<PathBuf>,
) -> Result<ExitCode, CliError> {
    match command {
        Command::Help => {
            print_help(false);
            Ok(ExitCode::SUCCESS)
        }
        Command::Version => {
            println!("mads {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        Command::Run(command) => {
            let root = current_dir.map_err(current_directory_error)?;
            let project = CargoProject::load(root)?;
            let target = project.resolve_application(&command.target)?;
            let built = cargo::build_application(&target).await?;
            let status = process::run_application(&built, &command.arguments).await?;

            match status.code() {
                Some(code @ 0..=255) => Ok(ExitCode::from(code as u8)),
                _ => Err(CliError::new(
                    MADS202,
                    "Application process failed",
                    "the selected application terminated without an ordinary exit code",
                )),
            }
        }
        Command::Inspect(command) => {
            let root = current_dir.map_err(current_directory_error)?;
            let project = CargoProject::load(root)?;
            let target = project.resolve_application(&command.target)?;
            let built = cargo::build_application(&target).await?;
            let _report = inspect_application(&built, command.kind).await?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Database(DatabaseInvocation {
            command: DatabaseCommand::Help,
            ..
        }) => {
            print_database_help(false);
            Ok(ExitCode::SUCCESS)
        }
        Command::Database(DatabaseInvocation { command, package }) => {
            run_database_command(command, package.as_deref(), current_dir).await
        }
    }
}

async fn run_database_command(
    command: DatabaseCommand,
    package: Option<&str>,
    current_dir: io::Result<PathBuf>,
) -> Result<ExitCode, CliError> {
    let root = current_dir.map_err(current_directory_error)?;
    let project = CargoProject::load(root)?;
    let package = project.resolve_package(package)?;

    match database::execute(command, package.package_root()).await {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            eprintln!("error: {error}");
            Ok(ExitCode::from(1))
        }
    }
}

fn current_directory_error(error: io::Error) -> CliError {
    CliError::new(
        MADS201,
        "Cargo project could not be loaded",
        "could not determine the invocation directory",
    )
    .with_source(error)
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

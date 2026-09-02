//! Value-preserving parsing for supported MADS.rs CLI commands.

use std::ffi::{OsStr, OsString};

use mads_common::__private::InspectionKind;

/// Cargo package and binary selectors for an application command.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TargetSelection {
    pub(crate) package: Option<String>,
    pub(crate) binary: Option<String>,
}

/// A command that selects and invokes an application binary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationCommand {
    pub(crate) target: TargetSelection,
    pub(crate) arguments: Vec<OsString>,
}

/// A command that requests a private application-inspection report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InspectionCommand {
    pub(crate) kind: InspectionKind,
    pub(crate) target: TargetSelection,
}

/// A database command and its selected Cargo package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DatabaseInvocation {
    pub(crate) command: DatabaseCommand,
    pub(crate) package: Option<String>,
}

/// A supported top-level command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    /// Prints general help.
    Help,
    /// Prints the CLI version.
    Version,
    /// Builds and runs an application.
    Run(ApplicationCommand),
    /// Watches, rebuilds, and restarts an application.
    Dev(ApplicationCommand),
    /// Inspects an application through its standard MADS entry point.
    Inspect(InspectionCommand),
    /// Runs or describes a database command.
    Database(DatabaseInvocation),
}

/// A supported database subcommand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DatabaseCommand {
    /// Applies pending migrations.
    Migrate,
    /// Reverts the most recently applied migration.
    Rollback,
    /// Prints migration status.
    Status,
    /// Prints database command help.
    Help,
}

/// An unsupported CLI argument form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ParseError {
    /// The top-level command was not recognized.
    UnknownCommand(OsString),
    /// An option or positional argument was not recognized.
    UnknownArgument(OsString),
    /// An option was not followed by its required value.
    MissingValue(&'static str),
    /// An option value could not be represented as Unicode.
    NonUnicodeValue(&'static str),
    /// An option was supplied more than once.
    DuplicateOption(&'static str),
    /// `db` was not followed by a database command.
    MissingDatabaseCommand,
    /// A database subcommand was not recognized.
    UnknownDatabaseCommand(OsString),
    /// A syntax error arose while parsing database command options.
    DatabaseSyntax(Box<ParseError>),
}

/// Parses process arguments after the executable name.
pub(crate) fn parse(arguments: &[OsString]) -> Result<Command, ParseError> {
    let Some((command, remaining)) = arguments.split_first() else {
        return Ok(Command::Help);
    };

    match command.to_str() {
        Some("--help" | "-h") if remaining.is_empty() => Ok(Command::Help),
        Some("--version" | "-V") if remaining.is_empty() => Ok(Command::Version),
        Some("run") => parse_application_command(remaining).map(Command::Run),
        Some("dev") => parse_application_command(remaining).map(Command::Dev),
        Some("routes") => parse_inspection_command(InspectionKind::Routes, remaining),
        Some("graph") => parse_inspection_command(InspectionKind::Graph, remaining),
        Some("doctor") => parse_inspection_command(InspectionKind::Doctor, remaining),
        Some("db") => parse_database_command(remaining),
        _ => Err(ParseError::UnknownCommand(command.clone())),
    }
}

fn parse_inspection_command(
    kind: InspectionKind,
    arguments: &[OsString],
) -> Result<Command, ParseError> {
    let mut target = TargetSelection::default();
    let mut index = 0;

    while let Some(argument) = arguments.get(index) {
        match argument.to_str() {
            Some("--package" | "-p") => {
                parse_selector(arguments, &mut index, "--package", &mut target.package)?;
            }
            Some("--bin") => {
                parse_selector(arguments, &mut index, "--bin", &mut target.binary)?;
            }
            _ => return Err(ParseError::UnknownArgument(argument.clone())),
        }
        index += 1;
    }

    Ok(Command::Inspect(InspectionCommand { kind, target }))
}

fn parse_application_command(arguments: &[OsString]) -> Result<ApplicationCommand, ParseError> {
    let mut target = TargetSelection::default();
    let mut index = 0;

    while let Some(argument) = arguments.get(index) {
        if argument == OsStr::new("--") {
            return Ok(ApplicationCommand {
                target,
                arguments: arguments[index + 1..].to_vec(),
            });
        }

        match argument.to_str() {
            Some("--package" | "-p") => {
                parse_selector(arguments, &mut index, "--package", &mut target.package)?;
            }
            Some("--bin") => {
                parse_selector(arguments, &mut index, "--bin", &mut target.binary)?;
            }
            _ => return Err(ParseError::UnknownArgument(argument.clone())),
        }
        index += 1;
    }

    Ok(ApplicationCommand {
        target,
        arguments: Vec::new(),
    })
}

fn parse_database_command(arguments: &[OsString]) -> Result<Command, ParseError> {
    let Some((command, options)) = arguments.split_first() else {
        return Err(ParseError::MissingDatabaseCommand);
    };

    let command = match command.to_str() {
        Some("migrate") => DatabaseCommand::Migrate,
        Some("rollback") => DatabaseCommand::Rollback,
        Some("status") => DatabaseCommand::Status,
        Some("--help" | "-h") => DatabaseCommand::Help,
        _ => return Err(ParseError::UnknownDatabaseCommand(command.clone())),
    };

    let mut package = None;
    let mut index = 0;
    while let Some(argument) = options.get(index) {
        match argument.to_str() {
            Some("--package" | "-p") => {
                parse_selector(options, &mut index, "--package", &mut package)
                    .map_err(|error| ParseError::DatabaseSyntax(Box::new(error)))?;
            }
            _ => {
                return Err(ParseError::DatabaseSyntax(Box::new(
                    ParseError::UnknownArgument(argument.clone()),
                )));
            }
        }
        index += 1;
    }

    Ok(Command::Database(DatabaseInvocation { command, package }))
}

fn parse_selector(
    arguments: &[OsString],
    index: &mut usize,
    option: &'static str,
    destination: &mut Option<String>,
) -> Result<(), ParseError> {
    if destination.is_some() {
        return Err(ParseError::DuplicateOption(option));
    }

    *index += 1;
    let value = arguments
        .get(*index)
        .ok_or(ParseError::MissingValue(option))?;
    let value = value.to_str().ok_or(ParseError::NonUnicodeValue(option))?;
    if value.starts_with('-') {
        return Err(ParseError::MissingValue(option));
    }
    *destination = Some(value.to_owned());
    Ok(())
}

impl ParseError {
    /// Returns whether the error arose while parsing a database command.
    pub(crate) fn is_database_command(&self) -> bool {
        match self {
            Self::MissingDatabaseCommand
            | Self::UnknownDatabaseCommand(_)
            | Self::DatabaseSyntax(_) => true,
            Self::UnknownCommand(_)
            | Self::UnknownArgument(_)
            | Self::MissingValue(_)
            | Self::NonUnicodeValue(_)
            | Self::DuplicateOption(_) => false,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownCommand(command) => {
                write!(formatter, "unknown command: {}", command.to_string_lossy())
            }
            Self::UnknownArgument(argument) => {
                write!(
                    formatter,
                    "unknown argument: {}",
                    argument.to_string_lossy()
                )
            }
            Self::MissingValue(option) => write!(formatter, "missing value for {option}"),
            Self::NonUnicodeValue(option) => {
                write!(formatter, "value for {option} is not valid Unicode")
            }
            Self::DuplicateOption(option) => write!(formatter, "duplicate option: {option}"),
            Self::MissingDatabaseCommand => write!(formatter, "missing database command"),
            Self::UnknownDatabaseCommand(command) => {
                write!(
                    formatter,
                    "unknown database command: {}",
                    command.to_string_lossy()
                )
            }
            Self::DatabaseSyntax(error) => error.fmt(formatter),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use mads_common::__private::InspectionKind;

    use super::{
        ApplicationCommand, Command, DatabaseInvocation, InspectionCommand, ParseError,
        TargetSelection, parse,
    };

    fn args(arguments: &[&str]) -> Vec<OsString> {
        arguments.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_run_selectors_and_preserves_forwarded_arguments() {
        let command = parse(&args(&[
            "run",
            "-p",
            "api",
            "--bin",
            "server",
            "--",
            "--port",
            "4100",
            "two words",
        ]))
        .unwrap();

        assert_eq!(
            command,
            Command::Run(ApplicationCommand {
                target: TargetSelection {
                    package: Some("api".into()),
                    binary: Some("server".into()),
                },
                arguments: args(&["--port", "4100", "two words"]),
            })
        );
    }

    #[test]
    fn parses_dev_selectors_and_preserves_forwarded_arguments() {
        let command = parse(&args(&[
            "dev",
            "-p",
            "api",
            "--bin",
            "server",
            "--",
            "--seed",
            "42",
            "two words",
        ]))
        .unwrap();

        assert_eq!(
            command,
            Command::Dev(ApplicationCommand {
                target: TargetSelection {
                    package: Some("api".into()),
                    binary: Some("server".into()),
                },
                arguments: args(&["--seed", "42", "two words"]),
            })
        );
    }

    #[test]
    fn dev_rejects_duplicate_and_missing_selectors_like_run() {
        for arguments in [
            args(&["--package", "api", "-p", "web"]),
            args(&["--bin"]),
            args(&["--package", "--bin", "server"]),
        ] {
            assert_eq!(
                parse(&[vec![OsString::from("dev")], arguments.clone()].concat()),
                parse(&[vec![OsString::from("run")], arguments].concat()),
            );
        }
    }

    #[test]
    fn parses_inspection_selectors_and_rejects_application_arguments() {
        assert_eq!(
            parse(&args(&["routes", "-p", "api", "--bin", "server"])).unwrap(),
            Command::Inspect(InspectionCommand {
                kind: InspectionKind::Routes,
                target: TargetSelection {
                    package: Some("api".into()),
                    binary: Some("server".into()),
                },
            })
        );
        assert!(parse(&args(&["doctor", "--", "argument"])).is_err());
    }

    #[test]
    fn database_commands_accept_package_but_reject_binary_and_application_arguments() {
        assert!(matches!(
            parse(&args(&["db", "status", "--package", "api"])),
            Ok(Command::Database(DatabaseInvocation { .. }))
        ));
        assert!(parse(&args(&["db", "status", "--bin", "server"])).is_err());
        assert!(parse(&args(&["db", "status", "--", "extra"])).is_err());
    }

    #[test]
    fn rejects_foundation_and_named_generation_forms() {
        assert!(parse(&args(&["foundation"])).is_err());
        assert!(parse(&args(&["db", "generate", "named"])).is_err());
        assert!(parse(&args(&["db", "generate", "--diff-schema"])).is_err());
    }

    #[test]
    fn rejects_duplicate_missing_and_non_unicode_selector_values() {
        assert!(matches!(
            parse(&args(&["run", "-p", "api", "--package", "web"])),
            Err(ParseError::DuplicateOption("--package"))
        ));
        assert!(matches!(
            parse(&args(&["run", "--bin"])),
            Err(ParseError::MissingValue("--bin"))
        ));
        assert!(matches!(
            parse(&args(&["run", "--package", "--bin", "server"])),
            Err(ParseError::MissingValue("--package"))
        ));
        assert!(matches!(
            parse(&args(&["db", "status", "--package", "--bin"])),
            Err(ParseError::DatabaseSyntax(error))
                if matches!(*error, ParseError::MissingValue("--package"))
        ));

        let mut arguments = args(&["run", "--package"]);
        arguments.push(non_unicode_argument());
        assert!(matches!(
            parse(&arguments),
            Err(ParseError::NonUnicodeValue("--package"))
        ));
    }

    #[test]
    fn rejects_unknown_top_level_and_database_arguments_precisely() {
        assert!(matches!(
            parse(&args(&["unknown"])),
            Err(ParseError::UnknownCommand(command)) if command == "unknown"
        ));
        assert!(matches!(
            parse(&args(&["run", "extra"])),
            Err(ParseError::UnknownArgument(argument)) if argument == "extra"
        ));
        assert!(matches!(
            parse(&args(&["db"])),
            Err(ParseError::MissingDatabaseCommand)
        ));
    }

    #[test]
    fn every_database_option_error_keeps_database_help_scope() {
        let cases = [
            args(&["db", "status", "--bin", "server"]),
            args(&["db", "status", "--package", "api", "-p", "web"]),
            args(&["db", "status", "--package"]),
        ];

        for arguments in cases {
            let error = parse(&arguments).unwrap_err();
            assert!(error.is_database_command(), "error lost DB scope: {error}");
        }

        let mut non_unicode = args(&["db", "status", "--package"]);
        non_unicode.push(non_unicode_argument());
        let error = parse(&non_unicode).unwrap_err();
        assert!(error.is_database_command(), "error lost DB scope: {error}");
    }

    #[test]
    fn preserves_non_unicode_application_arguments_after_separator() {
        let non_unicode = non_unicode_argument();
        let mut arguments = args(&["run", "--"]);
        arguments.push(non_unicode.clone());

        let Command::Run(command) = parse(&arguments).unwrap() else {
            panic!("run should parse as an application command");
        };

        assert_eq!(command.arguments, vec![non_unicode]);
    }

    #[cfg(unix)]
    fn non_unicode_argument() -> OsString {
        use std::os::unix::ffi::OsStringExt;

        OsString::from_vec(vec![0xff])
    }

    #[cfg(windows)]
    fn non_unicode_argument() -> OsString {
        use std::os::windows::ffi::OsStringExt;

        OsString::from_wide(&[0xd800])
    }
}

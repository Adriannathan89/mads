//! Fixed parsing for supported MADS.rs CLI commands.

/// A supported top-level command.
pub(crate) enum Command {
    /// Prints general help.
    Help,
    /// Prints the CLI version.
    Version,
    /// Prints the foundation report.
    Foundation,
    /// Runs or describes a database command.
    Database(DatabaseCommand),
}

/// A supported database subcommand.
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
pub(crate) enum ParseError {
    /// One or more arguments did not match the fixed grammar.
    UnknownArguments(Vec<String>),
    /// A database subcommand was not recognized.
    UnknownDatabaseCommand(String),
}

/// Parses process arguments after the executable name.
pub(crate) fn parse(arguments: &[String]) -> Result<Command, ParseError> {
    match arguments {
        [] => Ok(Command::Help),
        [argument] if matches!(argument.as_str(), "--help" | "-h") => Ok(Command::Help),
        [argument] if matches!(argument.as_str(), "--version" | "-V") => Ok(Command::Version),
        [argument] if argument == "foundation" => Ok(Command::Foundation),
        [argument] if argument == "db" => Ok(Command::Database(DatabaseCommand::Help)),
        [first, second] if first == "db" => parse_database_command(second),
        [first, ..] if first == "db" => Err(ParseError::UnknownArguments(arguments.to_vec())),
        _ => Err(ParseError::UnknownArguments(arguments.to_vec())),
    }
}

fn parse_database_command(argument: &str) -> Result<Command, ParseError> {
    let command = match argument {
        "migrate" => DatabaseCommand::Migrate,
        "rollback" => DatabaseCommand::Rollback,
        "status" => DatabaseCommand::Status,
        "--help" | "-h" => DatabaseCommand::Help,
        _ => return Err(ParseError::UnknownDatabaseCommand(argument.to_owned())),
    };
    Ok(Command::Database(command))
}

impl ParseError {
    /// Returns whether the error arose while parsing a database command.
    pub(crate) fn is_database_command(&self) -> bool {
        match self {
            Self::UnknownArguments(arguments) => {
                arguments.first().is_some_and(|argument| argument == "db")
            }
            Self::UnknownDatabaseCommand(_) => true,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownArguments(arguments) => {
                write!(formatter, "unknown argument(s): {}", arguments.join(" "))
            }
            Self::UnknownDatabaseCommand(command) => {
                write!(formatter, "unknown database command: {command}")
            }
        }
    }
}

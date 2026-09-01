use std::{ffi::OsString, process::Stdio};

use tokio::process::Command;

use crate::{
    cargo::BuiltApplication,
    diagnostic::{CliError, MADS202},
};

pub(crate) fn application_command(built: &BuiltApplication, arguments: &[OsString]) -> Command {
    let mut command = Command::new(built.executable());
    command
        .args(arguments)
        .current_dir(built.target().package().package_root())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    command
}

pub(crate) async fn run_application(
    built: &BuiltApplication,
    arguments: &[OsString],
) -> Result<std::process::ExitStatus, CliError> {
    let mut child = application_command(built, arguments)
        .spawn()
        .map_err(|error| {
            CliError::new(
                MADS202,
                "Application process failed",
                "could not start the selected application",
            )
            .with_source(error)
        })?;

    child.wait().await.map_err(|error| {
        CliError::new(
            MADS202,
            "Application process failed",
            "could not wait for the selected application",
        )
        .with_source(error)
    })
}

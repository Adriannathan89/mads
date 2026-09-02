use std::{
    ffi::OsString,
    future::{Future, pending},
    path::Path,
    pin::Pin,
    process::ExitCode,
    time::Duration,
};

use tokio::{
    sync::mpsc,
    time::{Instant, MissedTickBehavior},
};

use crate::{
    cargo::{BuiltApplication, build_application_owned},
    command::ApplicationCommand,
    dev_state::{DevAction, DevEvent, DevState},
    diagnostic::{CliError, MADS201, MADS220},
    process::{ApplicationProcess, StopOutcome, spawn_dev_application},
    project::{CargoProject, ResolvedApplication},
    watch::{ChangeImpact, WatchEvents, WatchSet},
};

const DEBOUNCE: Duration = Duration::from_millis(150);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

type DriverFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, CliError>> + Send + 'a>>;

trait DevDriver {
    fn build(&mut self, target: ResolvedApplication) -> DriverFuture<'static, BuiltApplication>;
    fn start(&mut self, built: BuiltApplication, arguments: Vec<OsString>) -> DriverFuture<'_, ()>;
    fn stop(&mut self) -> DriverFuture<'_, StopOutcome>;
    fn application_exited(&mut self) -> Result<bool, CliError>;
}

pub(crate) async fn run_dev(
    command: ApplicationCommand,
    root: &Path,
) -> Result<ExitCode, CliError> {
    let project = CargoProject::load(root)?;
    let target = project.resolve_application(&command.target)?;
    let watch_set = WatchSet::for_application(&project, &target);
    let events = WatchEvents::start(&watch_set)?;
    println!(
        "mads dev: watching {}/{}",
        target.package().package_name(),
        target.binary_name()
    );

    let mut driver = ProductionDevDriver::default();
    run_dev_with(command, &mut driver, watch_set, events).await?;
    Ok(ExitCode::SUCCESS)
}

async fn next_change(
    events: &mut WatchEvents,
    watch_set: &WatchSet,
) -> Result<ChangeImpact, CliError> {
    let mut impact = loop {
        let impact = watch_set.classify_event(&events.recv().await?);
        if impact != ChangeImpact::None {
            break impact;
        }
    };
    let mut deadline = Instant::now() + DEBOUNCE;

    loop {
        match tokio::time::timeout_at(deadline, events.recv()).await {
            Err(_) => return Ok(impact),
            Ok(event) => {
                let next = watch_set.classify_event(&event?);
                if next != ChangeImpact::None {
                    impact = ChangeImpact::merge([impact, next]);
                    deadline = Instant::now() + DEBOUNCE;
                }
            }
        }
    }
}

async fn run_dev_with<D: DevDriver>(
    command: ApplicationCommand,
    driver: &mut D,
    watch_set: WatchSet,
    events: WatchEvents,
) -> Result<(), CliError> {
    let root = std::env::current_dir().map_err(|error| {
        CliError::new(
            MADS201,
            "Cargo project could not be loaded",
            "could not determine the invocation directory",
        )
        .with_source(error)
    })?;
    let project = CargoProject::load(root)?;
    let target = project.resolve_application(&command.target)?;

    run_dev_with_target(command, driver, target, watch_set, events).await
}

async fn run_dev_with_target<D: DevDriver>(
    command: ApplicationCommand,
    driver: &mut D,
    target: ResolvedApplication,
    watch_set: WatchSet,
    events: WatchEvents,
) -> Result<(), CliError> {
    let (changes, mut changes_receiver) = mpsc::unbounded_channel();
    let watch_task = tokio::spawn(async move {
        let mut events = events;
        loop {
            let change = next_change(&mut events, &watch_set).await;
            let terminal = change.is_err();
            if changes.send(change).is_err() || terminal {
                break;
            }
        }
    });

    let result = run_dev_with_event_loop(command, driver, target, &mut changes_receiver, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await;
    if result.is_err() {
        let _ = driver.stop().await;
    }
    watch_task.abort();
    let _ = watch_task.await;
    result
}

async fn run_dev_with_event_loop<D, S>(
    command: ApplicationCommand,
    driver: &mut D,
    target: ResolvedApplication,
    changes: &mut mpsc::UnboundedReceiver<Result<ChangeImpact, CliError>>,
    shutdown: S,
) -> Result<(), CliError>
where
    D: DevDriver,
    S: Future<Output = ()>,
{
    tokio::pin!(shutdown);
    let mut state = DevState::new();
    let mut build = None;
    let mut process_checks = tokio::time::interval(PROCESS_POLL_INTERVAL);
    process_checks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut actions = state.transition(DevEvent::Start);

    loop {
        if execute_actions(
            &mut actions,
            driver,
            &target,
            &command.arguments,
            &mut build,
        )
        .await?
        {
            return Ok(());
        }

        tokio::select! {
            _ = &mut shutdown => {
                build = None;
                actions = state.transition(DevEvent::Shutdown);
            }
            change = changes.recv() => {
                let impact = change
                    .ok_or_else(watch_channel_closed)??;
                actions = state.transition(DevEvent::Changed(impact));
            }
            result = wait_for_build(&mut build) => {
                let result = result.expect("build future is only awaited while active");
                build = None;
                actions = match result {
                    Ok(built) => state.transition(DevEvent::BuildSucceeded(built)),
                    Err(error) => {
                        eprintln!("{error}");
                        eprintln!("mads dev: build failed; continuing to watch");
                        state.transition(DevEvent::BuildFailed)
                    }
                };
            }
            _ = process_checks.tick() => {
                if driver.application_exited()? {
                    eprintln!("mads dev: application exited; waiting for a relevant change");
                    actions = state.transition(DevEvent::ApplicationExited);
                }
            }
        }
    }
}

async fn wait_for_build(
    build: &mut Option<DriverFuture<'static, BuiltApplication>>,
) -> Option<Result<BuiltApplication, CliError>> {
    match build.as_mut() {
        Some(build) => Some(build.await),
        None => pending().await,
    }
}

async fn execute_actions<D: DevDriver>(
    actions: &mut Vec<DevAction>,
    driver: &mut D,
    target: &ResolvedApplication,
    arguments: &[OsString],
    build: &mut Option<DriverFuture<'static, BuiltApplication>>,
) -> Result<bool, CliError> {
    for action in std::mem::take(actions) {
        match action {
            DevAction::Build => {
                debug_assert!(build.is_none());
                eprintln!("mads dev: rebuilding {}", target.binary_name());
                *build = Some(driver.build(target.clone()));
            }
            DevAction::Start(built) => {
                eprintln!("mads dev: starting {}", built.target().binary_name());
                driver.start(built, arguments.to_vec()).await?;
            }
            DevAction::Stop => {
                eprintln!("mads dev: stopping application");
                let _ = driver.stop().await?;
            }
            DevAction::Restart(built) => {
                eprintln!("mads dev: restarting {}", built.target().binary_name());
                let _ = driver.stop().await?;
                driver.start(built, arguments.to_vec()).await?;
            }
            DevAction::Exit => {
                eprintln!("mads dev: exiting");
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn watch_channel_closed() -> CliError {
    CliError::new(
        MADS220,
        "File watcher failed",
        "the debounced watch event channel closed unexpectedly",
    )
}

#[derive(Default)]
struct ProductionDevDriver {
    application: Option<ApplicationProcess>,
}

impl DevDriver for ProductionDevDriver {
    fn build(&mut self, target: ResolvedApplication) -> DriverFuture<'static, BuiltApplication> {
        Box::pin(build_application_owned(target))
    }

    fn start(&mut self, built: BuiltApplication, arguments: Vec<OsString>) -> DriverFuture<'_, ()> {
        Box::pin(async move {
            if self.application.is_some() {
                return Err(CliError::new(
                    MADS220,
                    "Development supervisor failed",
                    "attempted to start a second application process",
                ));
            }
            self.application = Some(spawn_dev_application(&built, &arguments).await?);
            Ok(())
        })
    }

    fn stop(&mut self) -> DriverFuture<'_, StopOutcome> {
        Box::pin(async move {
            let Some(mut application) = self.application.take() else {
                return Ok(StopOutcome::Graceful);
            };
            application.stop(SHUTDOWN_TIMEOUT).await
        })
    }

    fn application_exited(&mut self) -> Result<bool, CliError> {
        let Some(application) = self.application.as_mut() else {
            return Ok(false);
        };
        if application.try_wait()?.is_some() {
            self.application = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
async fn run_scripted_dev(command: ApplicationCommand) -> Result<Vec<String>, CliError> {
    use std::collections::VecDeque;

    let target = ResolvedApplication::test_identity("scripted");
    let (changes, mut changes_receiver) = mpsc::unbounded_channel();
    let (shutdown, shutdown_receiver) = tokio::sync::oneshot::channel();
    let mut driver = ScriptedDevDriver {
        builds: VecDeque::from([
            (
                "v1",
                false,
                false,
                Ok(BuiltApplication::test_identity("v1")),
            ),
            (
                "v2",
                true,
                true,
                Err(CliError::new(
                    crate::diagnostic::MADS201,
                    "Cargo build failed",
                    "scripted failure",
                )),
            ),
            (
                "v3",
                false,
                false,
                Ok(BuiltApplication::test_identity("v3")),
            ),
        ]),
        operations: Vec::new(),
        running: None,
        stopped: None,
        changes,
        shutdown: Some(shutdown),
    };

    run_dev_with_event_loop(
        command,
        &mut driver,
        target,
        &mut changes_receiver,
        async move {
            let _ = shutdown_receiver.await;
        },
    )
    .await?;

    driver.finish();
    Ok(driver.operations)
}

#[cfg(test)]
struct ScriptedDevDriver {
    builds:
        std::collections::VecDeque<(&'static str, bool, bool, Result<BuiltApplication, CliError>)>,
    operations: Vec<String>,
    running: Option<String>,
    stopped: Option<String>,
    changes: mpsc::UnboundedSender<Result<ChangeImpact, CliError>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

#[cfg(test)]
impl ScriptedDevDriver {
    fn record_build_failure(&mut self) {
        let operation = match &self.running {
            Some(binary) => format!("build failed ({binary} remains running)"),
            None => "build failed".into(),
        };
        self.operations.push(operation);
    }

    fn finish(&mut self) {
        self.flush_stopped();
        self.operations.push("exit".into());
    }

    fn flush_stopped(&mut self) {
        if let Some(binary) = self.stopped.take() {
            self.operations.push(format!("stop {binary}"));
        }
    }
}

#[cfg(test)]
impl DevDriver for ScriptedDevDriver {
    fn build(&mut self, _target: ResolvedApplication) -> DriverFuture<'static, BuiltApplication> {
        let (version, record_running, queue_rebuild, result) =
            self.builds.pop_front().expect("a scripted build result");
        let operation = match (&self.running, record_running) {
            (Some(binary), true) => format!("build {version} ({binary} remains running)"),
            _ => format!("build {version}"),
        };
        self.operations.push(operation);
        if result.is_err() {
            self.record_build_failure();
        }
        let changes = self.changes.clone();
        Box::pin(async move {
            if queue_rebuild {
                let _ = changes.send(Ok(ChangeImpact::Rebuild));
            }
            result
        })
    }

    fn start(&mut self, built: BuiltApplication, arguments: Vec<OsString>) -> DriverFuture<'_, ()> {
        Box::pin(async move {
            let binary = built.target().binary_name().to_owned();
            let restarted = matches!(self.stopped.as_deref(), Some(stopped) if stopped == binary);
            match self.stopped.take() {
                Some(stopped) if stopped == binary => {
                    self.operations.push(format!("restart {binary}"));
                }
                Some(stopped) => {
                    self.operations.push(format!("stop {stopped}"));
                    self.operations.push(format!(
                        "start {binary} args=[{}]",
                        arguments
                            .iter()
                            .map(|argument| argument.to_string_lossy())
                            .collect::<Vec<_>>()
                            .join(",")
                    ));
                }
                None => self.operations.push(format!(
                    "start {binary} args=[{}]",
                    arguments
                        .iter()
                        .map(|argument| argument.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(",")
                )),
            }
            self.running = Some(binary);
            if restarted {
                let _ = self.changes.send(Ok(ChangeImpact::Rebuild));
            } else if self.running.as_deref() == Some("v1") {
                let _ = self.changes.send(Ok(ChangeImpact::Restart));
            } else if self.running.as_deref() == Some("v3") {
                let _ = self
                    .shutdown
                    .take()
                    .expect("the scripted application should shut down once")
                    .send(());
            }
            Ok(())
        })
    }

    fn stop(&mut self) -> DriverFuture<'_, StopOutcome> {
        Box::pin(async move {
            self.stopped = self.running.take();
            Ok(StopOutcome::Graceful)
        })
    }

    fn application_exited(&mut self) -> Result<bool, CliError> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use crate::command::{ApplicationCommand, TargetSelection};

    use super::run_scripted_dev;

    #[tokio::test]
    async fn scripted_driver_rebuilds_without_stopping_the_last_good_application() {
        let operations = run_scripted_dev(ApplicationCommand {
            target: TargetSelection::default(),
            arguments: vec![OsString::from("--seed"), OsString::from("42")],
        })
        .await
        .unwrap();

        assert_eq!(
            operations,
            vec![
                "build v1",
                "start v1 args=[--seed,42]",
                "restart v1",
                "build v2 (v1 remains running)",
                "build failed (v1 remains running)",
                "build v3",
                "stop v1",
                "start v3 args=[--seed,42]",
                "stop v3",
                "exit",
            ]
        );
    }
}

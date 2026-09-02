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
    cargo::{BuiltApplication, CargoBuild},
    command::ApplicationCommand,
    dev_state::{DevAction, DevEvent, DevState},
    diagnostic::{CliError, MADS220},
    process::{ApplicationProcess, StopOutcome, spawn_dev_application},
    project::{CargoProject, ResolvedApplication},
    watch::{ChangeImpact, WatchEvents, WatchSet},
};

const DEBOUNCE: Duration = Duration::from_millis(150);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

type DriverFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, CliError>> + Send + 'a>>;

trait DevDriver {
    fn build(&mut self, target: ResolvedApplication) -> Box<dyn BuildTask>;
    fn start(&mut self, built: BuiltApplication, arguments: Vec<OsString>) -> DriverFuture<'_, ()>;
    fn stop(&mut self) -> DriverFuture<'_, StopOutcome>;
    fn application_exited(&mut self) -> Result<bool, CliError>;
}

trait BuildTask: Send {
    fn wait(&mut self) -> DriverFuture<'_, BuiltApplication>;
    fn cancel_and_wait(&mut self) -> DriverFuture<'_, ()>;
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
    run_dev_with(command, &mut driver, root, watch_set, events, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
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

async fn run_dev_with<D, S>(
    command: ApplicationCommand,
    driver: &mut D,
    root: &Path,
    watch_set: WatchSet,
    events: WatchEvents,
    shutdown: S,
) -> Result<(), CliError>
where
    D: DevDriver,
    S: Future<Output = ()>,
{
    let project = CargoProject::load(root)?;
    let target = project.resolve_application(&command.target)?;

    run_dev_with_target(command, driver, target, watch_set, events, shutdown).await
}

async fn run_dev_with_target<D, S>(
    command: ApplicationCommand,
    driver: &mut D,
    target: ResolvedApplication,
    watch_set: WatchSet,
    events: WatchEvents,
    shutdown: S,
) -> Result<(), CliError>
where
    D: DevDriver,
    S: Future<Output = ()>,
{
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

    let result =
        run_dev_with_event_loop(command, driver, target, &mut changes_receiver, shutdown).await;
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

    let result = loop {
        let exited = match execute_actions(
            &mut actions,
            driver,
            &target,
            &command.arguments,
            &mut build,
        )
        .await
        {
            Ok(exited) => exited,
            Err(error) => break Err(error),
        };
        if exited {
            break Ok(());
        }

        tokio::select! {
            _ = &mut shutdown => {
                if let Err(error) = cancel_active_build(&mut build).await {
                    break Err(error);
                }
                actions = state.transition(DevEvent::Shutdown);
            }
            change = changes.recv() => {
                let impact = match change
                    .ok_or_else(watch_channel_closed)
                    .and_then(|change| change)
                {
                    Ok(impact) => impact,
                    Err(error) => break Err(error),
                };
                actions = state.transition(DevEvent::Changed(impact));
            }
            result = wait_for_build(&mut build) => {
                let result = result.expect("build future is only awaited while active");
                actions = match result {
                    Ok(built) => {
                        build = None;
                        state.transition(DevEvent::BuildSucceeded(built))
                    }
                    Err(error) => {
                        if let Err(cleanup) = cancel_active_build(&mut build).await {
                            break Err(cleanup);
                        }
                        eprintln!("{error}");
                        eprintln!("mads dev: build failed; continuing to watch");
                        state.transition(DevEvent::BuildFailed)
                    }
                };
            }
            _ = process_checks.tick() => {
                let application_exited = match driver.application_exited() {
                    Ok(application_exited) => application_exited,
                    Err(error) => break Err(error),
                };
                if application_exited {
                    eprintln!("mads dev: application exited; waiting for a relevant change");
                    actions = state.transition(DevEvent::ApplicationExited);
                }
            }
        }
    };

    if result.is_err() {
        let _ = cancel_active_build(&mut build).await;
        let _ = driver.stop().await;
    }
    result
}

async fn wait_for_build(
    build: &mut Option<Box<dyn BuildTask>>,
) -> Option<Result<BuiltApplication, CliError>> {
    match build.as_mut() {
        Some(build) => Some(build.wait().await),
        None => pending().await,
    }
}

async fn cancel_active_build(build: &mut Option<Box<dyn BuildTask>>) -> Result<(), CliError> {
    let Some(mut build) = build.take() else {
        return Ok(());
    };
    build.cancel_and_wait().await
}

async fn execute_actions<D: DevDriver>(
    actions: &mut Vec<DevAction>,
    driver: &mut D,
    target: &ResolvedApplication,
    arguments: &[OsString],
    build: &mut Option<Box<dyn BuildTask>>,
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
    fn build(&mut self, target: ResolvedApplication) -> Box<dyn BuildTask> {
        match CargoBuild::start(target) {
            Ok(build) => Box::new(build),
            Err(error) => Box::new(FailedBuild(Some(error))),
        }
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

struct FailedBuild(Option<CliError>);

impl BuildTask for FailedBuild {
    fn wait(&mut self) -> DriverFuture<'_, BuiltApplication> {
        let error = self.0.take().expect("failed builds are awaited once");
        Box::pin(async move { Err(error) })
    }

    fn cancel_and_wait(&mut self) -> DriverFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

impl BuildTask for CargoBuild {
    fn wait(&mut self) -> DriverFuture<'_, BuiltApplication> {
        Box::pin(self.wait())
    }

    fn cancel_and_wait(&mut self) -> DriverFuture<'_, ()> {
        Box::pin(self.cancel_and_wait())
    }
}

#[cfg(test)]
async fn run_scripted_dev(command: ApplicationCommand) -> Result<Vec<String>, CliError> {
    use std::collections::VecDeque;

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/run/single");
    let project = CargoProject::load(&root)?;
    let target = project.resolve_application(&command.target)?;
    let watch_set = WatchSet::for_application(&project, &target);
    let (events, receiver) = mpsc::unbounded_channel();
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
        events: events.clone(),
        package_root: target.package().package_root().to_path_buf(),
        shutdown: Some(shutdown),
    };

    let result = run_dev_with(
        command,
        &mut driver,
        &root,
        watch_set,
        WatchEvents::synthetic(receiver),
        async move {
            let _ = shutdown_receiver.await;
        },
    )
    .await;

    driver.finish();
    let operations = std::mem::take(&mut driver.operations);
    drop(driver);
    tokio::time::timeout(Duration::from_secs(1), events.closed())
        .await
        .expect("run_dev_with should await watcher-task cleanup");
    result?;
    Ok(operations)
}

#[cfg(test)]
struct ScriptedDevDriver {
    builds:
        std::collections::VecDeque<(&'static str, bool, bool, Result<BuiltApplication, CliError>)>,
    operations: Vec<String>,
    running: Option<String>,
    stopped: Option<String>,
    events: mpsc::UnboundedSender<Result<notify::Event, notify::Error>>,
    package_root: std::path::PathBuf,
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
    fn build(&mut self, _target: ResolvedApplication) -> Box<dyn BuildTask> {
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
        let events = self.events.clone();
        let source = self.package_root.join("src/main.rs");
        Box::new(ScriptedBuild {
            events,
            queue_rebuild,
            result: Some(result),
            source,
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
                let _ = self
                    .events
                    .send(Ok(modified_event(self.package_root.join("src/main.rs"))));
            } else if self.running.as_deref() == Some("v1") {
                let _ = self
                    .events
                    .send(Ok(modified_event(self.package_root.join("mads.toml"))));
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
struct ScriptedBuild {
    events: mpsc::UnboundedSender<Result<notify::Event, notify::Error>>,
    queue_rebuild: bool,
    result: Option<Result<BuiltApplication, CliError>>,
    source: std::path::PathBuf,
}

#[cfg(test)]
impl BuildTask for ScriptedBuild {
    fn wait(&mut self) -> DriverFuture<'_, BuiltApplication> {
        let result = self
            .result
            .take()
            .expect("scripted builds are awaited once");
        let events = self.events.clone();
        let source = self.source.clone();
        let queue_rebuild = self.queue_rebuild;
        Box::pin(async move {
            if queue_rebuild {
                let _ = events.send(Ok(modified_event(source)));
            }
            result
        })
    }

    fn cancel_and_wait(&mut self) -> DriverFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[cfg(test)]
fn modified_event(path: std::path::PathBuf) -> notify::Event {
    notify::Event::new(notify::EventKind::Modify(notify::event::ModifyKind::Any)).add_path(path)
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        future::pending,
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    use crate::{
        command::{ApplicationCommand, TargetSelection},
        process::StopOutcome,
        project::CargoProject,
        watch::{WatchEvents, WatchSet},
    };

    use super::{
        BuildTask, BuiltApplication, CliError, DevDriver, DriverFuture, ResolvedApplication,
        run_dev_with, run_scripted_dev,
    };

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

    #[tokio::test]
    async fn shutdown_awaits_the_active_build_cleanup() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/run/single");
        let project = CargoProject::load(&root).unwrap();
        let target = project
            .resolve_application(&TargetSelection::default())
            .unwrap();
        let watch_set = WatchSet::for_application(&project, &target);
        let (_events, receiver) = tokio::sync::mpsc::unbounded_channel();
        let cleaned = Arc::new(AtomicBool::new(false));
        let mut driver = CancellationDriver {
            cleaned: Arc::clone(&cleaned),
        };

        run_dev_with(
            ApplicationCommand {
                target: TargetSelection::default(),
                arguments: Vec::new(),
            },
            &mut driver,
            &root,
            watch_set,
            WatchEvents::synthetic(receiver),
            async {},
        )
        .await
        .unwrap();

        assert!(cleaned.load(Ordering::SeqCst));
    }

    struct CancellationDriver {
        cleaned: Arc<AtomicBool>,
    }

    impl DevDriver for CancellationDriver {
        fn build(&mut self, _target: ResolvedApplication) -> Box<dyn BuildTask> {
            Box::new(CancellationBuild {
                cleaned: Arc::clone(&self.cleaned),
            })
        }

        fn start(
            &mut self,
            _built: BuiltApplication,
            _arguments: Vec<OsString>,
        ) -> DriverFuture<'_, ()> {
            Box::pin(async { Ok(()) })
        }

        fn stop(&mut self) -> DriverFuture<'_, StopOutcome> {
            Box::pin(async { Ok(StopOutcome::Graceful) })
        }

        fn application_exited(&mut self) -> Result<bool, CliError> {
            Ok(false)
        }
    }

    struct CancellationBuild {
        cleaned: Arc<AtomicBool>,
    }

    impl BuildTask for CancellationBuild {
        fn wait(&mut self) -> DriverFuture<'_, BuiltApplication> {
            Box::pin(async { pending().await })
        }

        fn cancel_and_wait(&mut self) -> DriverFuture<'_, ()> {
            Box::pin(async move {
                self.cleaned.store(true, Ordering::SeqCst);
                Ok(())
            })
        }
    }
}

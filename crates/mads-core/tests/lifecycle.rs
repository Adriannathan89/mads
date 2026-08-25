//! Integration tests for application lifecycle ordering and rollback.

use std::sync::{Arc, Mutex};

use mads_core::{
    ApplicationContext, Config, Diagnostic, Error, LifecycleFuture, LifecycleHook,
    LifecycleManager, LifecycleState, MADS010, MADS011, MADS020,
};

struct RecordingHook {
    name: String,
    events: Arc<Mutex<Vec<String>>>,
    fail_start: bool,
    fail_stop: bool,
}

impl RecordingHook {
    fn new(name: &str, events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            name: name.to_owned(),
            events,
            fail_start: false,
            fail_stop: false,
        }
    }

    fn failing_start(name: &str, events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            name: name.to_owned(),
            events,
            fail_start: true,
            fail_stop: false,
        }
    }

    fn failing_stop(name: &str, events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            name: name.to_owned(),
            events,
            fail_start: false,
            fail_stop: true,
        }
    }
}

impl LifecycleHook for RecordingHook {
    fn name(&self) -> &str {
        &self.name
    }

    fn start<'a>(&'a self, _: &'a ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            self.events
                .lock()
                .expect("event recording lock should not be poisoned")
                .push(format!("start:{}", self.name));
            if self.fail_start {
                return Err(Error::new(Diagnostic::new(
                    MADS020,
                    "test startup failure",
                    "the test hook deliberately fails during startup",
                )));
            }
            Ok(())
        })
    }

    fn stop<'a>(&'a self, _: &'a ApplicationContext) -> LifecycleFuture<'a> {
        Box::pin(async move {
            self.events
                .lock()
                .expect("event recording lock should not be poisoned")
                .push(format!("stop:{}", self.name));
            if self.fail_stop {
                return Err(Error::new(Diagnostic::new(
                    MADS020,
                    "test rollback failure",
                    "the test hook deliberately fails during rollback",
                )));
            }
            Ok(())
        })
    }
}

#[tokio::test]
async fn starts_in_registration_order_and_stops_in_reverse_order() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let context = ApplicationContext::new(Default::default(), Config::empty());
    let mut lifecycle = LifecycleManager::new();
    lifecycle.add_hook(RecordingHook::new("database", Arc::clone(&events)));
    lifecycle.add_hook(RecordingHook::new("worker", Arc::clone(&events)));

    lifecycle.start(&context).await.expect("hooks should start");
    lifecycle
        .shutdown(&context)
        .await
        .expect("hooks should stop");

    assert_eq!(lifecycle.state(), LifecycleState::Stopped);
    assert_eq!(
        *events
            .lock()
            .expect("event recording lock should not be poisoned"),
        [
            "start:database",
            "start:worker",
            "stop:worker",
            "stop:database"
        ]
    );
}

#[tokio::test]
async fn rolls_back_started_hooks_and_retains_the_startup_error() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let context = ApplicationContext::new(Default::default(), Config::empty());
    let mut lifecycle = LifecycleManager::new();
    lifecycle.add_hook(RecordingHook::new("database", Arc::clone(&events)));
    lifecycle.add_hook(RecordingHook::failing_start("worker", Arc::clone(&events)));

    let error = lifecycle
        .start(&context)
        .await
        .expect_err("worker startup should fail");

    assert_eq!(error.code(), MADS011);
    assert!(error.to_string().contains("subject: worker"));
    let source = std::error::Error::source(&error).expect("startup cause should be retained");
    let original = source
        .downcast_ref::<Error>()
        .expect("startup cause should remain a framework error");
    assert_eq!(original.code(), MADS020);
    assert_eq!(lifecycle.state(), LifecycleState::Stopped);
    assert_eq!(
        *events
            .lock()
            .expect("event recording lock should not be poisoned"),
        ["start:database", "start:worker", "stop:database"]
    );
}

#[tokio::test]
async fn rejects_lifecycle_operations_from_invalid_states() {
    let context = ApplicationContext::new(Default::default(), Config::empty());
    let mut lifecycle = LifecycleManager::new();

    let shutdown_error = lifecycle
        .shutdown(&context)
        .await
        .expect_err("created applications cannot shut down");
    assert_eq!(shutdown_error.code(), MADS010);

    lifecycle
        .start(&context)
        .await
        .expect("application should start");
    lifecycle
        .shutdown(&context)
        .await
        .expect("application should shut down");
    let restart_error = lifecycle
        .start(&context)
        .await
        .expect_err("stopped applications cannot restart");
    assert_eq!(restart_error.code(), MADS010);
}

#[tokio::test]
async fn reports_rollback_failures_without_replacing_the_startup_cause() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let context = ApplicationContext::new(Default::default(), Config::empty());
    let mut lifecycle = LifecycleManager::new();
    lifecycle.add_hook(RecordingHook::failing_stop("database", Arc::clone(&events)));
    lifecycle.add_hook(RecordingHook::failing_start("worker", Arc::clone(&events)));

    let error = lifecycle
        .start(&context)
        .await
        .expect_err("worker startup should fail after database starts");

    assert_eq!(error.code(), MADS011);
    assert!(error.to_string().contains("subject: worker"));
    assert!(error.to_string().contains("rollback hook database failed"));
    assert!(error.to_string().contains("test rollback failure"));
    let source = std::error::Error::source(&error).expect("startup cause should be retained");
    let original = source
        .downcast_ref::<Error>()
        .expect("startup cause should remain a framework error");
    assert_eq!(original.code(), MADS020);
    assert!(original.to_string().contains("test startup failure"));
    assert_eq!(
        *events
            .lock()
            .expect("event recording lock should not be poisoned"),
        ["start:database", "start:worker", "stop:database"]
    );
}

#[tokio::test]
async fn infrastructure_sorts_by_owner_before_application_registration_order() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let context = ApplicationContext::new(Default::default(), Config::empty());
    let mut lifecycle = LifecycleManager::new();
    lifecycle.add_hook(RecordingHook::new("app-first", Arc::clone(&events)));
    lifecycle.add_infrastructure_hook(
        "zeta.infrastructure",
        RecordingHook::new("zeta", Arc::clone(&events)),
    );
    lifecycle.add_infrastructure_hook(
        "alpha.infrastructure",
        RecordingHook::new("alpha", Arc::clone(&events)),
    );
    lifecycle.add_hook(RecordingHook::new("app-second", Arc::clone(&events)));

    lifecycle.start(&context).await.unwrap();
    lifecycle.shutdown(&context).await.unwrap();

    assert_eq!(
        *events.lock().unwrap(),
        [
            "start:alpha",
            "start:zeta",
            "start:app-first",
            "start:app-second",
            "stop:app-second",
            "stop:app-first",
            "stop:zeta",
            "stop:alpha",
        ],
    );
}

#[tokio::test]
async fn failed_application_start_rolls_back_application_then_infrastructure() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let context = ApplicationContext::new(Default::default(), Config::empty());
    let mut lifecycle = LifecycleManager::new();
    lifecycle.add_hook(RecordingHook::new("app-ok", Arc::clone(&events)));
    lifecycle.add_infrastructure_hook(
        "mads.infrastructure",
        RecordingHook::new("database", Arc::clone(&events)),
    );
    lifecycle.add_hook(RecordingHook::failing_start(
        "app-fail",
        Arc::clone(&events),
    ));

    let error = lifecycle.start(&context).await.unwrap_err();

    assert_eq!(error.code(), MADS011);
    assert_eq!(
        *events.lock().unwrap(),
        [
            "start:database",
            "start:app-ok",
            "start:app-fail",
            "stop:app-ok",
            "stop:database",
        ],
    );
}

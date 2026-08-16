//! Integration tests for application lifecycle ordering and rollback.

use std::sync::{Arc, Mutex};

use mads_core::{
    ApplicationContext, Config, Diagnostic, Error, LifecycleFuture, LifecycleHook,
    LifecycleManager, LifecycleState, MADS011, MADS020,
};

struct RecordingHook {
    name: String,
    events: Arc<Mutex<Vec<String>>>,
    fail_start: bool,
}

impl RecordingHook {
    fn new(name: &str, events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            name: name.to_owned(),
            events,
            fail_start: false,
        }
    }

    fn failing_start(name: &str, events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            name: name.to_owned(),
            events,
            fail_start: true,
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

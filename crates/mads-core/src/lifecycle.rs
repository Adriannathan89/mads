//! Application lifecycle hooks, state transitions, and failure rollback.

use std::future::Future;
use std::pin::Pin;

use crate::{ApplicationContext, Diagnostic, Error, MADS010, MADS011, Result};

/// The lifecycle state of an application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleState {
    /// The application has been built but has not started.
    Created,
    /// The application is starting lifecycle hooks.
    Starting,
    /// The application has started successfully.
    Running,
    /// The application is stopping lifecycle hooks.
    Stopping,
    /// The application has stopped and cannot be restarted.
    Stopped,
}

/// The asynchronous result returned by a lifecycle hook.
pub type LifecycleFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/// Starts and stops an application resource.
pub trait LifecycleHook: Send + Sync {
    /// Returns a stable name used in lifecycle diagnostics.
    fn name(&self) -> &str;

    /// Starts this resource using the completed application context.
    fn start<'a>(&'a self, context: &'a ApplicationContext) -> LifecycleFuture<'a>;

    /// Stops this resource using the completed application context.
    fn stop<'a>(&'a self, context: &'a ApplicationContext) -> LifecycleFuture<'a>;
}

/// Coordinates ordered lifecycle hooks for one application.
pub struct LifecycleManager {
    state: LifecycleState,
    hooks: Vec<Box<dyn LifecycleHook>>,
}

impl LifecycleManager {
    /// Creates a manager in the created state with no lifecycle hooks.
    pub fn new() -> Self {
        Self {
            state: LifecycleState::Created,
            hooks: Vec::new(),
        }
    }

    /// Adds a lifecycle hook that starts after previously registered hooks.
    pub fn add_hook<H>(&mut self, hook: H) -> &mut Self
    where
        H: LifecycleHook + 'static,
    {
        self.hooks.push(Box::new(hook));
        self
    }

    /// Returns the manager's current lifecycle state.
    pub const fn state(&self) -> LifecycleState {
        self.state
    }

    /// Starts all hooks in registration order.
    #[allow(clippy::result_large_err)]
    pub async fn start(&mut self, context: &ApplicationContext) -> Result<()> {
        if self.state != LifecycleState::Created {
            return Err(invalid_transition(self.state, "start"));
        }

        self.state = LifecycleState::Starting;
        let mut started: Vec<usize> = Vec::new();

        for index in 0..self.hooks.len() {
            let hook = self.hooks[index].as_ref();
            if let Err(error) = hook.start(context).await {
                let startup_error = hook_failure(hook.name(), "startup", error);
                for started_index in started.into_iter().rev() {
                    let _: Result<()> = self.hooks[started_index].stop(context).await;
                }
                self.state = LifecycleState::Stopped;
                return Err(startup_error);
            }
            started.push(index);
        }

        self.state = LifecycleState::Running;
        Ok(())
    }

    /// Stops all hooks in reverse registration order.
    #[allow(clippy::result_large_err)]
    pub async fn shutdown(&mut self, context: &ApplicationContext) -> Result<()> {
        if self.state != LifecycleState::Running {
            return Err(invalid_transition(self.state, "shutdown"));
        }

        self.state = LifecycleState::Stopping;
        let mut failure = None;

        for hook in self.hooks.iter().rev() {
            if let Err(error) = hook.stop(context).await {
                if failure.is_none() {
                    failure = Some(hook_failure(hook.name(), "shutdown", error));
                }
            }
        }

        self.state = LifecycleState::Stopped;
        failure.map_or(Ok(()), Err)
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

fn invalid_transition(state: LifecycleState, operation: &str) -> Error {
    Error::new(
        Diagnostic::new(
            MADS010,
            "invalid lifecycle transition",
            format!("cannot {operation} an application while it is {state:?}"),
        )
        .with_subject(operation),
    )
}

fn hook_failure(name: &str, operation: &str, error: Error) -> Error {
    Error::with_source(
        Diagnostic::new(
            MADS011,
            "lifecycle hook failed",
            format!("lifecycle hook failed during {operation}"),
        )
        .with_subject(name),
        error,
    )
}

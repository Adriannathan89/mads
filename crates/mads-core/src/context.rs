//! Provider-resolution contexts for construction and running applications.

use std::sync::Arc;

use crate::{Config, ProviderRegistry, Result};

/// Borrows the providers and configuration available while constructing an application.
pub struct ConstructionContext<'a> {
    registry: &'a ProviderRegistry,
    config: &'a Config,
}

impl<'a> ConstructionContext<'a> {
    /// Creates a context over providers and configuration owned by the application builder.
    pub fn new(registry: &'a ProviderRegistry, config: &'a Config) -> Self {
        Self { registry, config }
    }

    /// Resolves a provider by its concrete type.
    #[allow(clippy::result_large_err)]
    pub fn resolve<T>(&self) -> Result<Arc<T>>
    where
        T: Send + Sync + 'static,
    {
        self.registry.resolve()
    }

    /// Returns the configuration available during construction.
    pub const fn config(&self) -> &'a Config {
        self.config
    }
}

/// Owns the immutable providers and configuration of a running application.
#[derive(Clone)]
pub struct ApplicationContext {
    registry: Arc<ProviderRegistry>,
    config: Arc<Config>,
}

impl ApplicationContext {
    /// Creates an immutable application context from completed construction state.
    pub fn new(registry: ProviderRegistry, config: Config) -> Self {
        Self {
            registry: Arc::new(registry),
            config: Arc::new(config),
        }
    }

    /// Resolves an application-scoped provider by its concrete type.
    #[allow(clippy::result_large_err)]
    pub fn resolve<T>(&self) -> Result<Arc<T>>
    where
        T: Send + Sync + 'static,
    {
        self.registry.resolve()
    }

    /// Returns the immutable application configuration.
    pub fn config(&self) -> &Config {
        self.config.as_ref()
    }
}

//! Explicit application construction and lifecycle ownership.

use crate::{
    ApplicationContext, Catalog, Config, ConstructionContext, LifecycleHook, LifecycleManager,
    LifecycleState, ProviderRegistry, Result,
};

/// Builds an application by explicitly providing and constructing providers.
pub struct MadsBuilder {
    config: Config,
    registry: ProviderRegistry,
    lifecycle: LifecycleManager,
}

impl MadsBuilder {
    /// Creates a builder with configuration available to provider constructors and resolvers.
    pub fn new(config: Config) -> Self {
        let mut registry = ProviderRegistry::new();
        registry
            .insert(config.clone())
            .expect("a new provider registry cannot already contain configuration");

        Self {
            config,
            registry,
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Provides a concrete application-scoped value.
    #[allow(clippy::result_large_err)]
    pub fn provide<T>(&mut self, value: T) -> Result<&mut Self>
    where
        T: Send + Sync + 'static,
    {
        self.registry.insert(value)?;
        Ok(self)
    }

    /// Constructs exactly one statically declared provider using currently provided dependencies.
    #[allow(clippy::result_large_err)]
    pub async fn construct<T>(&mut self) -> Result<&mut Self>
    where
        T: Send + Sync + 'static,
    {
        let descriptor = Catalog::provider_for::<T>()?;
        let value = {
            let context = ConstructionContext::new(&self.registry, &self.config);
            (descriptor.constructor())(&context).await?
        };

        self.registry
            .insert_erased(descriptor.type_id(), descriptor.type_name(), value)?;
        Ok(self)
    }

    /// Registers a hook that runs when the completed application starts and stops.
    pub fn lifecycle_hook<H>(&mut self, hook: H) -> &mut Self
    where
        H: LifecycleHook + 'static,
    {
        self.lifecycle.add_hook(hook);
        self
    }

    /// Finishes construction and returns the application in the created state.
    pub fn build(self) -> Mads {
        Mads {
            context: ApplicationContext::new(self.registry, self.config),
            lifecycle: self.lifecycle,
        }
    }
}

/// An explicitly constructed application and its lifecycle state.
pub struct Mads {
    context: ApplicationContext,
    lifecycle: LifecycleManager,
}

impl Mads {
    /// Creates a builder with empty configuration.
    pub fn builder() -> MadsBuilder {
        Self::builder_with_config(Config::empty())
    }

    /// Creates a builder with caller-supplied configuration.
    pub fn builder_with_config(config: Config) -> MadsBuilder {
        MadsBuilder::new(config)
    }

    /// Returns the application's current lifecycle state.
    pub const fn state(&self) -> LifecycleState {
        self.lifecycle.state()
    }

    /// Returns the immutable application context.
    pub const fn context(&self) -> &ApplicationContext {
        &self.context
    }

    /// Starts registered lifecycle hooks.
    #[allow(clippy::result_large_err)]
    pub async fn start(&mut self) -> Result<()> {
        self.lifecycle.start(&self.context).await
    }

    /// Stops registered lifecycle hooks in reverse registration order.
    #[allow(clippy::result_large_err)]
    pub async fn shutdown(&mut self) -> Result<()> {
        self.lifecycle.shutdown(&self.context).await
    }
}

//! Explicit application construction and lifecycle ownership.

use std::collections::VecDeque;

use crate::{
    ApplicationContext, ApplicationGraph, Catalog, Config, ConstructionContext, ConstructionPlan,
    ConstructionStep, Diagnostic, Error, GraphAnalysis, LifecycleHook, LifecycleManager,
    LifecycleState, MADS006, ProviderRegistry, Result,
    graph::{SatisfiedProvider, analyze_catalog},
};

/// Builds an application by explicitly providing and constructing providers.
pub struct MadsBuilder {
    config: Config,
    registry: ProviderRegistry,
    satisfied: Vec<SatisfiedProvider>,
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
            satisfied: vec![SatisfiedProvider::provided::<Config>()],
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
        self.satisfied.push(SatisfiedProvider::provided::<T>());
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
        self.satisfied
            .push(SatisfiedProvider::preconstructed::<T>());
        Ok(self)
    }

    /// Analyzes the complete provider graph without invoking constructors.
    pub fn analyze(&self) -> GraphAnalysis {
        analyze_catalog(&self.satisfied)
    }

    /// Registers a hook that runs when the completed application starts and stops.
    pub fn lifecycle_hook<H>(&mut self, hook: H) -> &mut Self
    where
        H: LifecycleHook + 'static,
    {
        self.lifecycle.add_hook(hook);
        self
    }

    /// Validates and automatically constructs the complete application graph.
    #[allow(clippy::result_large_err)]
    pub async fn build(mut self) -> Result<Mads> {
        let (graph, construction_plan) = self.analyze().into_valid_parts()?;

        for step in construction_plan.steps() {
            let value = {
                let context = ConstructionContext::new(&self.registry, &self.config);
                (step.descriptor().constructor())(&context)
                    .await
                    .map_err(|source| provider_construction_error(step, &graph, source))?
            };
            self.registry
                .insert_erased(step.type_id(), step.type_name, value)?;
        }

        Ok(Mads {
            context: ApplicationContext::new(self.registry, self.config),
            lifecycle: self.lifecycle,
            graph,
            construction_plan,
        })
    }
}

/// An explicitly constructed application and its lifecycle state.
pub struct Mads {
    context: ApplicationContext,
    lifecycle: LifecycleManager,
    graph: ApplicationGraph,
    construction_plan: ConstructionPlan,
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

    /// Returns the immutable graph validated before construction.
    pub const fn graph(&self) -> &ApplicationGraph {
        &self.graph
    }

    /// Returns the deterministic provider construction plan that was executed.
    pub const fn construction_plan(&self) -> &ConstructionPlan {
        &self.construction_plan
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

fn provider_construction_error(
    step: &ConstructionStep,
    graph: &ApplicationGraph,
    source: Error,
) -> Error {
    let mut diagnostic = Diagnostic::new(
        MADS006,
        "provider construction failed",
        "a provider constructor returned an error",
    )
    .with_subject(step.type_name())
    .with_location(step.location());
    if let Some(path) = consumer_path(graph, step) {
        diagnostic = diagnostic.with_suggestion(format!("construction path: {path}"));
    }
    Error::with_source(diagnostic, source)
}

fn consumer_path(graph: &ApplicationGraph, failing_step: &ConstructionStep) -> Option<String> {
    if !graph
        .dependencies
        .iter()
        .any(|edge| edge.dependency_type_id == failing_step.type_id())
    {
        return None;
    }

    let mut roots = graph
        .providers
        .iter()
        .enumerate()
        .filter(|(_, provider)| {
            !graph
                .dependencies
                .iter()
                .any(|edge| edge.dependency_type_id == provider.type_id)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    roots.sort_by(|left, right| provider_order(graph, *left, *right));

    let mut visited = vec![false; graph.providers.len()];
    let mut queue = VecDeque::new();
    for root in roots {
        visited[root] = true;
        queue.push_back(vec![root]);
    }

    while let Some(path) = queue.pop_front() {
        let provider = *path.last().expect("paths are never empty");
        if graph.providers[provider].type_id == failing_step.type_id() {
            return Some(
                path.iter()
                    .map(|index| graph.providers[*index].type_name)
                    .collect::<Vec<_>>()
                    .join(" -> "),
            );
        }

        let mut dependencies = graph
            .dependencies
            .iter()
            .filter(|edge| edge.provider_type_id == graph.providers[provider].type_id)
            .filter_map(|edge| {
                graph
                    .providers
                    .iter()
                    .position(|candidate| candidate.type_id == edge.dependency_type_id)
            })
            .collect::<Vec<_>>();
        dependencies.sort_by(|left, right| provider_order(graph, *left, *right));
        for dependency in dependencies {
            if !visited[dependency] {
                visited[dependency] = true;
                let mut path = path.clone();
                path.push(dependency);
                queue.push_back(path);
            }
        }
    }

    None
}

fn provider_order(graph: &ApplicationGraph, left: usize, right: usize) -> std::cmp::Ordering {
    let left = &graph.providers[left];
    let right = &graph.providers[right];
    left.type_name
        .cmp(right.type_name)
        .then_with(|| left.origin.cmp(&right.origin))
        .then_with(|| match (left.location, right.location) {
            (Some(left), Some(right)) => left
                .file
                .cmp(right.file)
                .then_with(|| left.line.cmp(&right.line))
                .then_with(|| left.column.cmp(&right.column)),
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        })
}

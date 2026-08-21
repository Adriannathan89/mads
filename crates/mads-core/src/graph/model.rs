//! Immutable records used to inspect a provider graph.

use std::any::TypeId;

use crate::{
    DependencyDescriptor, Diagnostic, ProviderDescriptor, ProviderKind, ProviderVisibility,
    SourceLocation,
};

/// Describes how a provider enters an application graph.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderOrigin {
    /// A value supplied directly by the application.
    Provided,
    /// A service declaration.
    Service,
    /// A repository declaration.
    Repository,
    /// A general provider declaration.
    Provider,
}

impl From<ProviderKind> for ProviderOrigin {
    fn from(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::Service => Self::Service,
            ProviderKind::Repository => Self::Repository,
            ProviderKind::Provider => Self::Provider,
        }
    }
}

/// Describes how a provider value is satisfied for a specific build.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderState {
    /// A value supplied directly by the application.
    Provided,
    /// A value manually constructed before automatic construction.
    Preconstructed,
    /// A statically declared provider awaiting construction.
    Planned,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) struct SatisfiedProvider {
    pub(crate) type_id: TypeId,
    pub(crate) type_name: &'static str,
    pub(crate) state: ProviderState,
}

#[allow(dead_code)]
impl SatisfiedProvider {
    pub(crate) fn provided<T>() -> Self
    where
        T: Send + Sync + 'static,
    {
        Self {
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
            state: ProviderState::Provided,
        }
    }

    pub(crate) fn preconstructed<T>() -> Self
    where
        T: Send + Sync + 'static,
    {
        Self {
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
            state: ProviderState::Preconstructed,
        }
    }
}

/// Immutable metadata for one provider in an application graph.
pub struct ProviderNode {
    pub(crate) type_id: TypeId,
    pub(crate) type_name: &'static str,
    pub(crate) origin: ProviderOrigin,
    pub(crate) visibility: ProviderVisibility,
    pub(crate) state: ProviderState,
    pub(crate) location: Option<SourceLocation>,
    pub(crate) declared_dependencies: &'static [DependencyDescriptor],
}

impl ProviderNode {
    /// Returns the provider's stable output type name.
    pub const fn type_name(&self) -> &str {
        self.type_name
    }

    /// Returns how the provider enters the application graph.
    pub const fn origin(&self) -> ProviderOrigin {
        self.origin
    }

    /// Returns the provider declaration's visibility metadata.
    pub const fn visibility(&self) -> ProviderVisibility {
        self.visibility
    }

    /// Returns the provider's current satisfaction state.
    pub const fn state(&self) -> ProviderState {
        self.state
    }

    /// Returns the provider declaration location when it is statically declared.
    pub const fn location(&self) -> Option<SourceLocation> {
        self.location
    }

    /// Returns the provider's declared dependencies in source declaration order.
    pub const fn declared_dependencies(&self) -> &[DependencyDescriptor] {
        self.declared_dependencies
    }
}

/// A resolved dependency from one provider to a required type.
pub struct DependencyEdge {
    #[allow(dead_code)]
    pub(crate) provider_type_id: TypeId,
    pub(crate) provider_type_name: &'static str,
    #[allow(dead_code)]
    pub(crate) dependency_type_id: TypeId,
    pub(crate) dependency_type_name: &'static str,
}

impl DependencyEdge {
    /// Returns the stable type name of the provider declaring the dependency.
    pub const fn provider_type_name(&self) -> &str {
        self.provider_type_name
    }

    /// Returns the stable type name of the required dependency.
    pub const fn dependency_type_name(&self) -> &str {
        self.dependency_type_name
    }
}

/// An immutable ordered provider graph.
pub struct ApplicationGraph {
    pub(crate) providers: Vec<ProviderNode>,
    pub(crate) dependencies: Vec<DependencyEdge>,
}

impl ApplicationGraph {
    /// Returns providers in deterministic graph order.
    pub fn providers(&self) -> &[ProviderNode] {
        &self.providers
    }

    /// Returns resolved dependency edges in deterministic graph order.
    pub fn dependencies(&self) -> &[DependencyEdge] {
        &self.dependencies
    }

    /// Returns the provider node for `T`, if the graph contains one.
    pub fn provider<T>(&self) -> Option<&ProviderNode>
    where
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        self.providers
            .iter()
            .find(|provider| provider.type_id == type_id)
    }
}

/// One statically declared provider in construction order.
pub struct ConstructionStep {
    #[allow(dead_code)]
    pub(crate) type_id: TypeId,
    pub(crate) type_name: &'static str,
    pub(crate) origin: ProviderOrigin,
    pub(crate) location: SourceLocation,
    #[allow(dead_code)]
    pub(crate) descriptor: &'static ProviderDescriptor,
}

impl ConstructionStep {
    /// Returns the stable output type name of the provider to construct.
    pub const fn type_name(&self) -> &str {
        self.type_name
    }

    /// Returns how the provider enters the application graph.
    pub const fn origin(&self) -> ProviderOrigin {
        self.origin
    }

    /// Returns the source location of the provider declaration.
    pub const fn location(&self) -> SourceLocation {
        self.location
    }
}

/// A deterministic sequence of providers to construct.
pub struct ConstructionPlan {
    pub(crate) steps: Vec<ConstructionStep>,
}

impl ConstructionPlan {
    /// Returns construction steps in deterministic execution order.
    pub fn steps(&self) -> &[ConstructionStep] {
        &self.steps
    }
}

/// The result of graph inspection and validation.
pub struct GraphAnalysis {
    pub(crate) graph: ApplicationGraph,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) construction_plan: Option<ConstructionPlan>,
}

impl GraphAnalysis {
    /// Returns the immutable provider graph.
    pub const fn graph(&self) -> &ApplicationGraph {
        &self.graph
    }

    /// Returns diagnostics emitted while analyzing the graph.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns the deterministic construction plan when the graph is valid.
    pub const fn construction_plan(&self) -> Option<&ConstructionPlan> {
        self.construction_plan.as_ref()
    }

    /// Reports whether analysis produced a construction plan without diagnostics.
    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty() && self.construction_plan.is_some()
    }
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::sync::Arc;

    use crate::{
        ConstructionContext, ErasedProvider, ProviderDescriptor, ProviderFuture, ProviderKind,
        ProviderVisibility, SourceLocation,
    };

    use super::*;

    struct Database;
    struct Repository;

    #[test]
    fn immutable_graph_types_expose_stable_metadata() {
        let database = ProviderNode {
            type_id: TypeId::of::<Database>(),
            type_name: "graph::Database",
            origin: ProviderOrigin::Provided,
            visibility: ProviderVisibility::Public,
            state: ProviderState::Provided,
            location: None,
            declared_dependencies: &[],
        };
        let graph = ApplicationGraph {
            providers: vec![database],
            dependencies: Vec::new(),
        };
        let analysis = GraphAnalysis {
            graph,
            diagnostics: Vec::new(),
            construction_plan: Some(ConstructionPlan { steps: Vec::new() }),
        };

        assert!(analysis.is_valid());
        assert_eq!(analysis.graph().providers().len(), 1);
        assert_eq!(
            analysis.graph().provider::<Database>().unwrap().origin(),
            ProviderOrigin::Provided,
        );
        assert_eq!(
            analysis
                .graph()
                .provider::<Database>()
                .unwrap()
                .visibility(),
            ProviderVisibility::Public,
        );
        assert!(analysis.construction_plan().unwrap().steps().is_empty());
    }

    fn repository_type_id() -> TypeId {
        TypeId::of::<Repository>()
    }

    fn repository_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
        Box::pin(async { Ok(Arc::new(Repository) as ErasedProvider) })
    }

    static REPOSITORY_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
        ProviderKind::Repository,
        "graph::Repository",
        repository_type_id,
        &[],
        ProviderVisibility::Private,
        SourceLocation::new("repository.rs", 3, 1),
        repository_constructor,
    );

    #[test]
    fn edges_and_steps_expose_stable_metadata() {
        let edge = DependencyEdge {
            provider_type_id: TypeId::of::<Repository>(),
            provider_type_name: "graph::Repository",
            dependency_type_id: TypeId::of::<Database>(),
            dependency_type_name: "graph::Database",
        };
        let step = ConstructionStep {
            type_id: TypeId::of::<Repository>(),
            type_name: "graph::Repository",
            origin: ProviderOrigin::Repository,
            location: SourceLocation::new("repository.rs", 3, 1),
            descriptor: &REPOSITORY_DESCRIPTOR,
        };

        assert_eq!(edge.provider_type_name(), "graph::Repository");
        assert_eq!(edge.dependency_type_name(), "graph::Database");
        assert_eq!(step.type_name(), "graph::Repository");
        assert_eq!(step.origin(), ProviderOrigin::Repository);
        assert_eq!(step.location(), SourceLocation::new("repository.rs", 3, 1));
    }
}

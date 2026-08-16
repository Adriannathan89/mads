//! Static metadata contracts emitted by provider and module declarations.

use std::any::TypeId;
use std::future::Future;
use std::pin::Pin;

use crate::{ConstructionContext, ErasedProvider, Result, SourceLocation};

/// Categorizes the role a provider plays in an application.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderKind {
    /// A provider implementing application service behavior.
    Service,
    /// A provider implementing persistence access behavior.
    Repository,
    /// A general-purpose provider.
    Provider,
}

/// The asynchronous result of constructing an erased provider.
pub type ProviderFuture<'a> = Pin<Box<dyn Future<Output = Result<ErasedProvider>> + Send + 'a>>;

/// Constructs a provider using the dependencies and configuration available at startup.
pub type ProviderConstructor = for<'a> fn(&'a ConstructionContext<'a>) -> ProviderFuture<'a>;

/// Describes a provider dependency by its stable type metadata.
pub struct DependencyDescriptor {
    type_name: &'static str,
    type_id: fn() -> TypeId,
}

impl DependencyDescriptor {
    /// Creates a dependency descriptor from a stable type name and identifier factory.
    pub const fn new(type_name: &'static str, type_id: fn() -> TypeId) -> Self {
        Self { type_name, type_id }
    }

    /// Returns the dependency's stable type name.
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Returns the dependency's runtime type identifier.
    pub fn type_id(&self) -> TypeId {
        (self.type_id)()
    }
}

/// Describes a statically declared provider and its constructor.
pub struct ProviderDescriptor {
    kind: ProviderKind,
    type_name: &'static str,
    type_id: fn() -> TypeId,
    dependencies: &'static [DependencyDescriptor],
    location: SourceLocation,
    constructor: ProviderConstructor,
}

impl ProviderDescriptor {
    /// Creates a provider descriptor from static declaration metadata.
    pub const fn new(
        kind: ProviderKind,
        type_name: &'static str,
        type_id: fn() -> TypeId,
        dependencies: &'static [DependencyDescriptor],
        location: SourceLocation,
        constructor: ProviderConstructor,
    ) -> Self {
        Self {
            kind,
            type_name,
            type_id,
            dependencies,
            location,
            constructor,
        }
    }

    /// Returns the provider's role.
    pub const fn kind(&self) -> ProviderKind {
        self.kind
    }

    /// Returns the provider's stable output type name.
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Returns the provider's runtime output type identifier.
    pub fn type_id(&self) -> TypeId {
        (self.type_id)()
    }

    /// Returns the static dependency descriptors required by this provider.
    pub const fn dependencies(&self) -> &'static [DependencyDescriptor] {
        self.dependencies
    }

    /// Returns the provider declaration's source location.
    pub const fn location(&self) -> SourceLocation {
        self.location
    }

    /// Returns the provider constructor.
    pub const fn constructor(&self) -> ProviderConstructor {
        self.constructor
    }
}

/// Describes a statically declared application module.
pub struct ModuleDescriptor {
    type_name: &'static str,
    type_id: fn() -> TypeId,
    location: SourceLocation,
}

impl ModuleDescriptor {
    /// Creates a module descriptor from static declaration metadata.
    pub const fn new(
        type_name: &'static str,
        type_id: fn() -> TypeId,
        location: SourceLocation,
    ) -> Self {
        Self {
            type_name,
            type_id,
            location,
        }
    }

    /// Returns the module's stable type name.
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Returns the module's runtime type identifier.
    pub fn type_id(&self) -> TypeId {
        (self.type_id)()
    }

    /// Returns the module declaration's source location.
    pub const fn location(&self) -> SourceLocation {
        self.location
    }
}

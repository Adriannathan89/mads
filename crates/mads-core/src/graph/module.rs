//! Immutable module graph records and deterministic rooted traversal.

use std::any::TypeId;
use std::collections::HashSet;

use crate::{Diagnostic, Error, MADS008, ModuleDescriptor, Result, SourceLocation};

/// A reachable module in a rooted application graph.
pub struct ModuleNode {
    type_id: TypeId,
    type_name: &'static str,
    namespace: &'static str,
    location: SourceLocation,
}

impl ModuleNode {
    /// Returns the module's runtime type identifier.
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the module's stable Rust type name.
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Returns the Rust namespace owned by this module.
    pub const fn namespace(&self) -> &'static str {
        self.namespace
    }

    /// Returns the module declaration's source location.
    pub const fn location(&self) -> SourceLocation {
        self.location
    }
}

/// A direct, authored import between two reachable modules.
pub struct ModuleImportEdge {
    importer: usize,
    imported: usize,
}

impl ModuleImportEdge {
    /// Returns the module declaring this import.
    pub fn importer<'a>(&self, graph: &'a ModuleGraph) -> &'a ModuleNode {
        &graph.modules[self.importer]
    }

    /// Returns the directly imported module.
    pub fn imported<'a>(&self, graph: &'a ModuleGraph) -> &'a ModuleNode {
        &graph.modules[self.imported]
    }
}

/// Associates a selected provider with its owning reachable module, if any.
pub struct ProviderOwnership {
    pub(crate) provider_type_name: &'static str,
    pub(crate) module_type_name: Option<&'static str>,
}

impl ProviderOwnership {
    /// Returns the selected provider's stable type name.
    pub const fn provider_type_name(&self) -> &'static str {
        self.provider_type_name
    }

    /// Returns the owning module type name, or `None` for an unowned provider.
    pub const fn module_type_name(&self) -> Option<&'static str> {
        self.module_type_name
    }
}

/// The deterministic module scope retained for a rooted application.
pub struct ModuleGraph {
    root: usize,
    modules: Vec<ModuleNode>,
    imports: Vec<ModuleImportEdge>,
    provider_ownership: Vec<ProviderOwnership>,
}

impl ModuleGraph {
    /// Returns the selected root module.
    pub fn root(&self) -> &ModuleNode {
        &self.modules[self.root]
    }

    /// Returns reachable modules in deterministic declaration-order traversal.
    pub fn modules(&self) -> &[ModuleNode] {
        &self.modules
    }

    /// Returns every direct import edge in deterministic traversal order.
    pub fn imports(&self) -> &[ModuleImportEdge] {
        &self.imports
    }

    /// Returns selected provider ownership records ordered by provider type name.
    pub fn provider_ownership(&self) -> &[ProviderOwnership] {
        &self.provider_ownership
    }

    /// Finds the nearest reachable module namespace that contains `namespace`.
    #[doc(hidden)]
    pub fn owner_for_namespace(&self, namespace: &str) -> Option<&ModuleNode> {
        self.modules
            .iter()
            .filter(|module| namespace_contains(module.namespace(), namespace))
            .max_by_key(|module| module.namespace().len())
    }

    /// Returns whether `importer` directly imports `imported`.
    #[doc(hidden)]
    pub fn directly_imports(&self, importer: TypeId, imported: TypeId) -> bool {
        self.imports.iter().any(|edge| {
            edge.importer(self).type_id() == importer && edge.imported(self).type_id() == imported
        })
    }

    #[allow(dead_code)]
    pub(crate) fn is_reachable(&self, module: TypeId) -> bool {
        self.modules
            .iter()
            .any(|candidate| candidate.type_id() == module)
    }

    #[allow(dead_code)]
    pub(crate) fn set_provider_ownership(&mut self, ownership: Vec<ProviderOwnership>) {
        self.provider_ownership = ownership;
    }
}

pub(crate) fn build_module_graph(
    root_type_id: TypeId,
    modules: &[&'static ModuleDescriptor],
) -> Result<ModuleGraph> {
    let mut state = BuildState {
        descriptors: modules,
        visited: HashSet::new(),
        stack: Vec::new(),
        modules: Vec::new(),
        imports: Vec::new(),
    };
    visit(root_type_id, &mut state)?;
    let root = state
        .node_index(root_type_id)
        .expect("successful traversal records the root module");

    Ok(ModuleGraph {
        root,
        modules: state.modules,
        imports: state.imports,
        provider_ownership: Vec::new(),
    })
}

pub(crate) fn validate_module_catalog(modules: &[&'static ModuleDescriptor]) -> Result<()> {
    for (index, descriptor) in modules.iter().copied().enumerate() {
        let Some(namespace) = descriptor.namespace() else {
            continue;
        };
        let conflicts = modules
            .iter()
            .copied()
            .enumerate()
            .filter(|(candidate_index, candidate)| {
                *candidate_index != index && candidate.namespace() == Some(namespace)
            })
            .map(|(_, candidate)| candidate)
            .collect::<Vec<_>>();
        if !conflicts.is_empty() {
            return Err(module_namespace_error(descriptor, &conflicts));
        }
    }

    Ok(())
}

fn visit(module: TypeId, state: &mut BuildState<'_>) -> Result<()> {
    if let Some(cycle_start) = state.stack.iter().position(|id| *id == module) {
        return Err(state.module_cycle_error(&state.stack[cycle_start..], module));
    }
    if !state.visited.insert(module) {
        return Ok(());
    }

    let descriptor = state.require_descriptor(module)?;
    state.validate_unique_namespace(descriptor)?;
    let node = state.push_node(descriptor)?;
    state.stack.push(module);
    state.validate_duplicate_direct_imports(descriptor)?;
    for import in descriptor.imports() {
        let imported = state.require_descriptor(import.type_id())?;
        let imported_node = state.ensure_node(imported)?;
        state.push_edge(node, imported_node);
        visit(import.type_id(), state)?;
    }
    state.stack.pop();
    Ok(())
}

struct BuildState<'a> {
    descriptors: &'a [&'static ModuleDescriptor],
    visited: HashSet<TypeId>,
    stack: Vec<TypeId>,
    modules: Vec<ModuleNode>,
    imports: Vec<ModuleImportEdge>,
}

impl BuildState<'_> {
    fn require_descriptor(&self, type_id: TypeId) -> Result<&'static ModuleDescriptor> {
        let mut matches = self
            .descriptors
            .iter()
            .copied()
            .filter(|descriptor| descriptor.type_id() == type_id)
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| {
            left.type_name()
                .cmp(right.type_name())
                .then_with(|| location_order(left.location(), right.location()))
        });

        match matches.as_slice() {
            [] => Err(Error::new(
                Diagnostic::new(
                    MADS008,
                    "missing module metadata",
                    "a rooted module or direct import has no static module declaration",
                )
                .with_subject("requested module"),
            )),
            [descriptor] => Ok(*descriptor),
            [first, rest @ ..] => {
                let primary = Diagnostic::new(
                    MADS008,
                    "ambiguous module metadata",
                    "a module type has more than one static declaration",
                )
                .with_subject(first.type_name())
                .with_location(first.location());
                let related = rest.iter().map(|descriptor| {
                    Diagnostic::new(
                        MADS008,
                        "conflicting module declaration",
                        "this declaration describes the same module type",
                    )
                    .with_subject(descriptor.type_name())
                    .with_location(descriptor.location())
                });
                Err(Error::from_diagnostics(primary, related))
            }
        }
    }

    fn validate_unique_namespace(&self, descriptor: &ModuleDescriptor) -> Result<()> {
        let Some(namespace) = descriptor.namespace() else {
            return Err(Error::new(
                Diagnostic::new(
                    MADS008,
                    "missing module namespace",
                    "rooted module graphs require namespace metadata",
                )
                .with_subject(descriptor.type_name())
                .with_location(descriptor.location()),
            ));
        };

        let conflicts = self
            .descriptors
            .iter()
            .copied()
            .filter(|candidate| {
                candidate.type_id() != descriptor.type_id()
                    && candidate.namespace() == Some(namespace)
            })
            .collect::<Vec<_>>();
        if !conflicts.is_empty() {
            return Err(module_namespace_error(descriptor, &conflicts));
        }

        Ok(())
    }

    fn validate_duplicate_direct_imports(&self, descriptor: &ModuleDescriptor) -> Result<()> {
        let mut seen = Vec::new();
        for import in descriptor.imports() {
            let imported = import.type_id();
            if seen.contains(&imported) {
                return Err(Error::new(
                    Diagnostic::new(
                        MADS008,
                        "duplicate direct module import",
                        format!(
                            "module `{}` appears more than once in the direct import list",
                            import.type_name()
                        ),
                    )
                    .with_subject(descriptor.type_name())
                    .with_location(descriptor.location()),
                ));
            }
            seen.push(imported);
        }
        Ok(())
    }

    fn push_node(&mut self, descriptor: &ModuleDescriptor) -> Result<usize> {
        self.ensure_node(descriptor)
    }

    fn ensure_node(&mut self, descriptor: &ModuleDescriptor) -> Result<usize> {
        if let Some(index) = self.node_index(descriptor.type_id()) {
            return Ok(index);
        }
        let Some(namespace) = descriptor.namespace() else {
            return Err(Error::new(
                Diagnostic::new(
                    MADS008,
                    "missing module namespace",
                    "rooted module graphs require namespace metadata",
                )
                .with_subject(descriptor.type_name())
                .with_location(descriptor.location()),
            ));
        };
        let index = self.modules.len();
        self.modules.push(ModuleNode {
            type_id: descriptor.type_id(),
            type_name: descriptor.type_name(),
            namespace,
            location: descriptor.location(),
        });
        Ok(index)
    }

    fn push_edge(&mut self, importer: usize, imported: usize) {
        self.imports.push(ModuleImportEdge { importer, imported });
    }

    fn node_index(&self, type_id: TypeId) -> Option<usize> {
        self.modules
            .iter()
            .position(|module| module.type_id() == type_id)
    }

    fn module_cycle_error(&self, cycle: &[TypeId], repeated: TypeId) -> Error {
        let mut chain = cycle
            .iter()
            .filter_map(|type_id| self.descriptor_for_stack(*type_id))
            .collect::<Vec<_>>();
        if let Some(repeated) = self.descriptor_for_stack(repeated) {
            chain.push(repeated);
        }
        let subject = chain
            .iter()
            .map(|descriptor| descriptor.type_name())
            .collect::<Vec<_>>()
            .join(" -> ");
        let primary_descriptor = chain
            .first()
            .expect("cycle members were already resolved during traversal");
        let primary = Diagnostic::new(
            MADS008,
            "module import cycle",
            "reachable module imports form a cycle",
        )
        .with_subject(subject.clone())
        .with_location(primary_descriptor.location());
        let related = chain
            .iter()
            .skip(1)
            .take(chain.len().saturating_sub(2))
            .map(|descriptor| {
                Diagnostic::new(
                    MADS008,
                    "module in import cycle",
                    "this module participates in the import cycle",
                )
                .with_subject(subject.clone())
                .with_location(descriptor.location())
            });
        Error::from_diagnostics(primary, related)
    }

    fn descriptor_for_stack(&self, type_id: TypeId) -> Option<&'static ModuleDescriptor> {
        self.descriptors
            .iter()
            .copied()
            .find(|descriptor| descriptor.type_id() == type_id)
    }
}

fn module_namespace_error(descriptor: &ModuleDescriptor, conflicts: &[&ModuleDescriptor]) -> Error {
    let namespace = descriptor
        .namespace()
        .expect("namespace collision candidates have namespace metadata");
    let primary = Diagnostic::new(
        MADS008,
        "ambiguous module namespace",
        "multiple module declarations claim the same Rust namespace",
    )
    .with_subject(namespace)
    .with_location(descriptor.location());
    let related = conflicts.iter().map(|conflict| {
        Diagnostic::new(
            MADS008,
            "conflicting module namespace declaration",
            "this module declaration claims the same namespace",
        )
        .with_subject(namespace)
        .with_location(conflict.location())
    });
    Error::from_diagnostics(primary, related)
}

fn namespace_contains(parent: &str, child: &str) -> bool {
    child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with("::"))
}

fn location_order(left: SourceLocation, right: SourceLocation) -> std::cmp::Ordering {
    left.file
        .cmp(right.file)
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.column.cmp(&right.column))
}

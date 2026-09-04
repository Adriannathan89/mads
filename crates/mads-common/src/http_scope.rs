//! HTTP metadata selected for one application build.

use std::any::TypeId;

use mads_core::{Catalog, Mads, ModuleDescriptor, ModuleGraph, Result};

use crate::{
    ControllerRouteDescriptor, HttpMethod, RouteCatalog, RouteContractDescriptor, RouteDescriptor,
};

#[cfg(feature = "jwt")]
use crate::{GuardCatalog, GuardDescriptor};

/// A selected Passport guard and the module context that selected it.
#[cfg(feature = "jwt")]
pub(crate) struct ScopedGuard {
    guard: &'static GuardDescriptor,
    context_module: Option<TypeId>,
}

#[cfg(feature = "jwt")]
impl ScopedGuard {
    pub(crate) const fn guard(&self) -> &'static GuardDescriptor {
        self.guard
    }

    pub(crate) const fn context_module(&self) -> Option<TypeId> {
        self.context_module
    }
}

/// A static identity for one route emitted by one controller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RouteIdentity {
    controller: TypeId,
    method: HttpMethod,
    full_path: &'static str,
    handler: &'static str,
    #[cfg(feature = "jwt")]
    passport_context_module: Option<TypeId>,
}

impl RouteIdentity {
    fn new(controller: &ControllerRouteDescriptor, route: &RouteDescriptor) -> Self {
        Self {
            controller: controller.type_id(),
            method: route.method(),
            full_path: route.full_path(),
            handler: route.handler(),
            #[cfg(feature = "jwt")]
            passport_context_module: None,
        }
    }

    #[cfg(feature = "jwt")]
    fn with_passport_context_module(mut self, context_module: Option<TypeId>) -> Self {
        self.passport_context_module = context_module;
        self
    }

    pub(crate) fn matches(
        &self,
        controller: &ControllerRouteDescriptor,
        route: &RouteDescriptor,
    ) -> bool {
        self.controller == controller.type_id()
            && self.method == route.method()
            && self.full_path == route.full_path()
            && self.handler == route.handler()
    }

    #[cfg(feature = "jwt")]
    const fn passport_context_module(&self) -> Option<TypeId> {
        self.passport_context_module
    }
}

/// A controller selected for one HTTP application, including its selected routes.
pub(crate) struct ScopedController {
    descriptor: &'static ControllerRouteDescriptor,
    selected_routes: Vec<RouteIdentity>,
    context_module: Option<TypeId>,
}

impl ScopedController {
    pub(crate) const fn descriptor(&self) -> &'static ControllerRouteDescriptor {
        self.descriptor
    }

    pub(crate) fn selects(&self, route: &RouteDescriptor) -> bool {
        self.selected_routes
            .iter()
            .any(|identity| identity.matches(self.descriptor, route))
    }

    #[cfg(feature = "jwt")]
    pub(crate) fn passport_context_module(&self, route: &RouteDescriptor) -> Option<TypeId> {
        self.selected_routes
            .iter()
            .find(|identity| identity.matches(self.descriptor, route))
            .and_then(RouteIdentity::passport_context_module)
    }

    fn has_routes(&self) -> bool {
        !self.selected_routes.is_empty()
    }

    #[allow(dead_code)]
    pub(crate) const fn context_module(&self) -> Option<TypeId> {
        self.context_module
    }
}

/// The HTTP controller, route, and guard metadata selected for one application.
pub(crate) struct HttpApplicationScope {
    controllers: Vec<ScopedController>,
    #[cfg(feature = "jwt")]
    guards: Vec<ScopedGuard>,
}

impl HttpApplicationScope {
    pub(crate) fn for_application(application: &Mads) -> Result<Self> {
        Self::for_module_graph(application.module_graph())
    }

    pub(crate) fn for_module_graph(module_graph: Option<&ModuleGraph>) -> Result<Self> {
        let controllers = match module_graph {
            None => Self::complete_controllers(),
            Some(graph) => Self::rooted_controllers(graph),
        };

        #[cfg(feature = "jwt")]
        let guards = Self::selected_guards(module_graph, &controllers);

        Ok(Self {
            controllers,
            #[cfg(feature = "jwt")]
            guards,
        })
    }

    /// Creates an inspection-only scope rooted in a successfully analyzed module graph.
    ///
    /// Unlike complete-catalog runtime selection, missing rooted analysis deliberately
    /// produces an empty scope so inspection cannot expose unrelated linked metadata.
    #[allow(dead_code)] // Used by the private inspection path added in the next task.
    pub(crate) fn for_rooted_inspection(module_graph: Option<&ModuleGraph>) -> Result<Self> {
        let controllers = module_graph
            .map(Self::rooted_controllers)
            .unwrap_or_default();
        #[cfg(feature = "jwt")]
        let guards = match module_graph {
            Some(graph) => Self::selected_guards(Some(graph), &controllers),
            None => Vec::new(),
        };
        Ok(Self {
            controllers,
            #[cfg(feature = "jwt")]
            guards,
        })
    }

    pub(crate) fn controllers(&self) -> &[ScopedController] {
        &self.controllers
    }

    pub(crate) fn has_routes(&self) -> bool {
        self.controllers.iter().any(ScopedController::has_routes)
    }

    /// Iterates selected static route metadata without constructing controllers.
    #[allow(dead_code)] // Used by the private inspection path added in the next task.
    pub(crate) fn route_records(
        &self,
    ) -> impl Iterator<
        Item = (
            &ControllerRouteDescriptor,
            &RouteContractDescriptor,
            &RouteDescriptor,
        ),
    > {
        self.controllers.iter().flat_map(|controller| {
            controller
                .descriptor()
                .contracts()
                .iter()
                .flat_map(move |contract| {
                    contract
                        .routes()
                        .iter()
                        .filter(move |route| controller.selects(route))
                        .map(move |route| (controller.descriptor(), contract, route))
                })
        })
    }

    #[cfg(feature = "jwt")]
    pub(crate) fn guards(&self) -> &[ScopedGuard] {
        &self.guards
    }

    fn complete_controllers() -> Vec<ScopedController> {
        RouteCatalog::controllers()
            .into_iter()
            .map(|descriptor| ScopedController {
                descriptor,
                selected_routes: descriptor
                    .contracts()
                    .iter()
                    .flat_map(|contract| contract.routes())
                    .map(|route| RouteIdentity::new(descriptor, route))
                    .collect(),
                context_module: None,
            })
            .collect()
    }

    fn rooted_controllers(graph: &ModuleGraph) -> Vec<ScopedController> {
        let mut controllers = Vec::new();

        for descriptor in RouteCatalog::controllers() {
            let Some(owner) = owner_for_namespace(descriptor.namespace()) else {
                continue;
            };
            if !is_reachable(graph, owner.type_id()) {
                continue;
            }
            let context_module = Some(owner.type_id());
            let mut selected_routes = Vec::new();

            for contract in descriptor.contracts() {
                for route in contract.routes() {
                    let route_context = route_context(graph, context_module, route);
                    if route_context.is_some() {
                        #[cfg(feature = "jwt")]
                        let passport_context_module = route.guard().and_then(|guard| {
                            owner_for_namespace(guard.namespace())
                                .filter(|owner| is_reachable(graph, owner.type_id()))
                                .map(ModuleDescriptor::type_id)
                                .or(route_context)
                        });
                        let route = RouteIdentity::new(descriptor, route);
                        #[cfg(feature = "jwt")]
                        let route = route.with_passport_context_module(passport_context_module);
                        selected_routes.push(route);
                    }
                }
            }

            controllers.push(ScopedController {
                descriptor,
                selected_routes,
                context_module,
            });
        }

        controllers
    }

    #[cfg(feature = "jwt")]
    fn selected_guards(
        module_graph: Option<&ModuleGraph>,
        controllers: &[ScopedController],
    ) -> Vec<ScopedGuard> {
        if module_graph.is_none() {
            return GuardCatalog::guards()
                .into_iter()
                .map(|guard| ScopedGuard {
                    guard,
                    context_module: None,
                })
                .collect();
        };

        controllers
            .iter()
            .flat_map(|controller| {
                controller
                    .descriptor()
                    .contracts()
                    .iter()
                    .flat_map(|contract| contract.routes())
                    .filter(move |route| controller.selects(route))
                    .filter_map(move |route| {
                        route.guard().map(|guard| ScopedGuard {
                            guard,
                            context_module: controller.passport_context_module(route),
                        })
                    })
            })
            .collect()
    }
}

fn route_context(
    graph: &ModuleGraph,
    controller_context: Option<TypeId>,
    route: &RouteDescriptor,
) -> Option<TypeId> {
    match owner_for_namespace(route.namespace()) {
        Some(owner) if is_reachable(graph, owner.type_id()) => Some(owner.type_id()),
        Some(_) => None,
        None => controller_context,
    }
}

pub(crate) fn owner_for_namespace(namespace: Option<&str>) -> Option<&'static ModuleDescriptor> {
    let namespace = namespace?;
    Catalog::modules()
        .into_iter()
        .filter(|module| {
            module
                .namespace()
                .is_some_and(|owner| namespace_contains(owner, namespace))
        })
        .max_by_key(|module| module.namespace().map_or(0, str::len))
}

fn is_reachable(graph: &ModuleGraph, module: TypeId) -> bool {
    graph
        .modules()
        .iter()
        .any(|candidate| candidate.type_id() == module)
}

fn namespace_contains(parent: &str, child: &str) -> bool {
    child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with("::"))
}

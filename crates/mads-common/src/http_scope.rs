//! HTTP metadata selected for one application build.

use std::any::TypeId;

use mads_core::{Catalog, Mads, ModuleDescriptor, ModuleGraph, Result};

use crate::{ControllerRouteDescriptor, HttpMethod, RouteCatalog, RouteDescriptor};

#[cfg(feature = "jwt")]
use crate::{GuardCatalog, GuardDescriptor};

/// A selected Passport guard and the module context that selected it.
#[cfg(feature = "jwt")]
#[allow(dead_code)]
pub(crate) struct ScopedGuard {
    guard: &'static GuardDescriptor,
    context_module: Option<TypeId>,
}

#[cfg(feature = "jwt")]
#[allow(dead_code)]
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
}

impl RouteIdentity {
    fn new(controller: &ControllerRouteDescriptor, route: &RouteDescriptor) -> Self {
        Self {
            controller: controller.type_id(),
            method: route.method(),
            full_path: route.full_path(),
            handler: route.handler(),
        }
    }

    pub(crate) fn matches(
        &self,
        controller: &ControllerRouteDescriptor,
        route: &RouteDescriptor,
    ) -> bool {
        *self == Self::new(controller, route)
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

    #[allow(dead_code)]
    pub(crate) const fn context_module(&self) -> Option<TypeId> {
        self.context_module
    }
}

/// The HTTP controller, route, and guard metadata selected for one application.
pub(crate) struct HttpApplicationScope {
    controllers: Vec<ScopedController>,
    #[cfg(feature = "jwt")]
    #[allow(dead_code)]
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

    pub(crate) fn controllers(&self) -> &[ScopedController] {
        &self.controllers
    }

    #[cfg(feature = "jwt")]
    #[allow(dead_code)]
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
                        selected_routes.push(RouteIdentity::new(descriptor, route));
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
        let Some(graph) = module_graph else {
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
                        route.guard().map(|guard| {
                            let route_context =
                                route_context(graph, controller.context_module(), route);
                            let context_module = owner_for_namespace(guard.namespace())
                                .filter(|owner| is_reachable(graph, owner.type_id()))
                                .map(ModuleDescriptor::type_id)
                                .or(route_context);
                            ScopedGuard {
                                guard,
                                context_module,
                            }
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

//! Framework-neutral route metadata and validation.

use std::any::TypeId;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use mads_core::{Diagnostic, Error, MADS030, Result, SourceLocation};

/// An HTTP method declared by a route contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HttpMethod {
    /// The HTTP GET method.
    Get,
    /// The HTTP POST method.
    Post,
    /// The HTTP PUT method.
    Put,
    /// The HTTP PATCH method.
    Patch,
    /// The HTTP DELETE method.
    Delete,
}

impl HttpMethod {
    /// Returns the conventional uppercase HTTP method name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

/// Static metadata for one route method.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RouteDescriptor {
    method: HttpMethod,
    prefix: &'static str,
    path: &'static str,
    full_path: &'static str,
    handler: &'static str,
    location: SourceLocation,
}

impl RouteDescriptor {
    /// Creates static route metadata emitted by `#[routes]`.
    pub const fn new(
        method: HttpMethod,
        prefix: &'static str,
        path: &'static str,
        full_path: &'static str,
        handler: &'static str,
        location: SourceLocation,
    ) -> Self {
        Self {
            method,
            prefix,
            path,
            full_path,
            handler,
            location,
        }
    }

    /// Returns the HTTP method.
    pub const fn method(self) -> HttpMethod {
        self.method
    }

    /// Returns the route-trait prefix, or an empty string when no prefix exists.
    pub const fn prefix(self) -> &'static str {
        self.prefix
    }

    /// Returns the endpoint path as declared on the method.
    pub const fn path(self) -> &'static str {
        self.path
    }

    /// Returns the canonical path formed from prefix and endpoint path.
    pub const fn full_path(self) -> &'static str {
        self.full_path
    }

    /// Returns the route-contract method name.
    pub const fn handler(self) -> &'static str {
        self.handler
    }

    /// Returns the source location of the declaring route contract.
    pub const fn location(self) -> SourceLocation {
        self.location
    }
}

/// Route metadata contributed by one route trait to a controller.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RouteContractDescriptor {
    trait_name: &'static str,
    routes: &'static [RouteDescriptor],
}

impl RouteContractDescriptor {
    /// Creates route-contract metadata emitted by `#[controller]`.
    pub const fn new(trait_name: &'static str, routes: &'static [RouteDescriptor]) -> Self {
        Self { trait_name, routes }
    }

    /// Returns the declared route trait name.
    pub const fn trait_name(self) -> &'static str {
        self.trait_name
    }

    /// Returns the routes supplied by this contract.
    pub const fn routes(self) -> &'static [RouteDescriptor] {
        self.routes
    }
}

/// Static route metadata contributed by one managed controller.
pub struct ControllerRouteDescriptor {
    type_name: &'static str,
    type_id: fn() -> TypeId,
    contracts: &'static [RouteContractDescriptor],
}

impl ControllerRouteDescriptor {
    /// Creates static controller-route metadata emitted by `#[controller]`.
    pub const fn new(
        type_name: &'static str,
        type_id: fn() -> TypeId,
        contracts: &'static [RouteContractDescriptor],
    ) -> Self {
        Self {
            type_name,
            type_id,
            contracts,
        }
    }

    /// Returns the controller type name.
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Returns the controller type identifier.
    pub fn type_id(&self) -> TypeId {
        (self.type_id)()
    }

    /// Returns route contracts implemented by the controller.
    pub const fn contracts(&self) -> &'static [RouteContractDescriptor] {
        self.contracts
    }
}

mads_core::__private::inventory::collect!(ControllerRouteDescriptor);

/// Looks up and validates route-contract metadata.
pub struct RouteCatalog;

impl RouteCatalog {
    /// Returns registered controllers in deterministic type-name order.
    pub fn controllers() -> Vec<&'static ControllerRouteDescriptor> {
        controller_cache().clone()
    }

    /// Returns route descriptors declared by controller `T`, preserving trait and method order.
    pub fn routes_for<T>() -> Vec<RouteDescriptor>
    where
        T: Send + Sync + 'static,
    {
        Self::controllers()
            .into_iter()
            .find(|controller| controller.type_id() == TypeId::of::<T>())
            .into_iter()
            .flat_map(|controller| controller.contracts().iter().copied())
            .flat_map(|contract| contract.routes().iter().copied())
            .collect()
    }

    /// Validates route conflicts within controller `T`.
    #[allow(clippy::result_large_err)]
    pub fn validate_controller<T>() -> Result<()>
    where
        T: Send + Sync + 'static,
    {
        let Some(controller) = Self::controllers()
            .into_iter()
            .find(|controller| controller.type_id() == TypeId::of::<T>())
        else {
            return Ok(());
        };

        validate_routes(
            controller.type_name(),
            routes_for_descriptor(controller).map(|route| (controller.type_name(), route)),
        )
    }

    /// Validates route conflicts across every registered controller.
    #[allow(clippy::result_large_err)]
    pub fn validate() -> Result<()> {
        let routes = Self::controllers().into_iter().flat_map(|controller| {
            routes_for_descriptor(controller).map(move |route| (controller.type_name(), route))
        });
        validate_routes("application", routes)
    }
}

fn controller_cache() -> &'static Vec<&'static ControllerRouteDescriptor> {
    static CONTROLLERS: OnceLock<Vec<&'static ControllerRouteDescriptor>> = OnceLock::new();
    CONTROLLERS.get_or_init(|| {
        let mut controllers: Vec<_> =
            mads_core::__private::inventory::iter::<ControllerRouteDescriptor>
                .into_iter()
                .collect();
        controllers.sort_by_key(|controller| controller.type_name());
        controllers
    })
}

fn routes_for_descriptor(
    controller: &'static ControllerRouteDescriptor,
) -> impl Iterator<Item = RouteDescriptor> {
    controller
        .contracts()
        .iter()
        .copied()
        .flat_map(|contract| contract.routes().iter().copied())
}

fn validate_routes(
    scope: &'static str,
    routes: impl IntoIterator<Item = (&'static str, RouteDescriptor)>,
) -> Result<()> {
    let mut seen = BTreeMap::new();
    for (controller, route) in routes {
        let key = (route.method(), canonical_pattern(route.full_path()));
        if let Some((previous_controller, previous_route)) = seen.insert(key, (controller, route)) {
            let primary = Diagnostic::new(
                MADS030,
                "conflicting routes",
                format!(
                    "{} {} is declared by both `{previous_controller}` and `{controller}` in `{scope}`",
                    route.method().as_str(),
                    route.full_path(),
                ),
            )
            .with_subject(format!("{} {}", route.method().as_str(), route.full_path()))
            .with_location(route.location())
            .with_suggestion("use a unique HTTP method and canonical path for each controller route");
            let related = Diagnostic::new(
                MADS030,
                "first route declaration",
                "the conflicting route was first declared here",
            )
            .with_subject(previous_controller)
            .with_location(previous_route.location());
            return Err(Error::from_diagnostics(primary, [related]));
        }
    }
    Ok(())
}

fn canonical_pattern(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.starts_with(':') {
                ":*"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

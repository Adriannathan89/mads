//! HTTP route metadata and validation for the common Axum adapter.
//!
//! The types in this module are the stable boundary between compile-time route
//! macros and the v0.3 HTTP adapter. They contain only immutable, `'static`
//! metadata, so catalog inspection does not construct controllers or start a
//! server. [`RouteCatalog`] provides deterministic lookup and conflict
//! validation over the descriptors registered by `#[controller]`.

use std::any::TypeId;
use std::collections::BTreeMap;
#[cfg(feature = "jwt")]
use std::fmt;
#[cfg(feature = "jwt")]
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use mads_core::{Diagnostic, Error, MADS030, Result, SourceLocation};

use crate::http_scope::ScopedController;

#[cfg(feature = "jwt")]
use crate::passport::{GuardDescriptor, PassportGuardState, PassportStrategyPreflight};

#[cfg(feature = "jwt")]
#[derive(Clone, Copy)]
struct GuardReference(&'static crate::passport::GuardDescriptor);

#[cfg(feature = "jwt")]
impl PartialEq for GuardReference {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

#[cfg(feature = "jwt")]
impl Eq for GuardReference {}

#[cfg(feature = "jwt")]
impl Hash for GuardReference {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.0 as *const crate::passport::GuardDescriptor).hash(state);
    }
}

#[cfg(feature = "jwt")]
impl fmt::Debug for GuardReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GuardReference")
            .field("route_trait", &self.0.route_trait())
            .field("handler", &self.0.handler())
            .finish()
    }
}

/// An HTTP method declared by a route contract.
///
/// This enum mirrors the route attributes exported by the common integration
/// (`#[get]`, `#[post]`, `#[put]`, `#[patch]`, and `#[delete]`).
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
    ///
    /// # Examples
    ///
    /// ```
    /// use mads_common::HttpMethod;
    ///
    /// assert_eq!(HttpMethod::Get.as_str(), "GET");
    /// assert_eq!(HttpMethod::Delete.as_str(), "DELETE");
    /// ```
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
///
/// A descriptor records both the method-local path and the canonical path after
/// applying the route-trait prefix. Its source location points back to the
/// route declaration, allowing catalog diagnostics to identify the offending
/// source span without retaining runtime state.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RouteDescriptor {
    method: HttpMethod,
    prefix: &'static str,
    path: &'static str,
    full_path: &'static str,
    handler: &'static str,
    namespace: Option<&'static str>,
    location: SourceLocation,
    #[cfg(feature = "jwt")]
    guard: Option<GuardReference>,
}

impl RouteDescriptor {
    /// Creates static route metadata emitted by `#[routes]`.
    ///
    /// Integrations that construct descriptors manually receive the same
    /// fail-closed validation as macro-generated metadata when they pass the
    /// descriptor to [`RouteCatalog::validate`] or
    /// [`crate::__private::validate_descriptors`]. `full_path` must be the
    /// canonical join of `prefix` and `path`.
    ///
    /// # Examples
    ///
    /// ```
    /// use mads_common::{HttpMethod, RouteDescriptor};
    /// use mads_common::core::SourceLocation;
    ///
    /// const ROUTE: RouteDescriptor = RouteDescriptor::new(
    ///     HttpMethod::Get,
    ///     "/users",
    ///     "/:id",
    ///     "/users/:id",
    ///     "get_user",
    ///     SourceLocation::new("routes.rs", 10, 5),
    /// );
    ///
    /// assert_eq!(ROUTE.full_path(), "/users/:id");
    /// ```
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
            namespace: None,
            location,
            #[cfg(feature = "jwt")]
            guard: None,
        }
    }

    /// Attaches the Rust namespace containing this route declaration.
    #[doc(hidden)]
    #[must_use]
    pub const fn with_namespace(mut self, namespace: &'static str) -> Self {
        self.namespace = Some(namespace);
        self
    }

    /// Returns the Rust namespace containing this route declaration, when available.
    #[doc(hidden)]
    pub const fn namespace(&self) -> Option<&'static str> {
        self.namespace
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

    /// Associates this route with one effective Passport guard descriptor.
    ///
    /// Route expansion uses the exact same static descriptor for catalog
    /// inspection and future request-time enforcement.
    #[cfg(feature = "jwt")]
    #[doc(hidden)]
    #[must_use]
    pub const fn with_guard(mut self, guard: &'static crate::passport::GuardDescriptor) -> Self {
        self.guard = Some(GuardReference(guard));
        self
    }

    /// Returns the effective Passport guard for this route when it has one.
    #[cfg(feature = "jwt")]
    #[doc(hidden)]
    #[must_use]
    pub const fn guard(&self) -> Option<&'static crate::passport::GuardDescriptor> {
        match self.guard {
            Some(guard) => Some(guard.0),
            None => None,
        }
    }
}

/// Route metadata contributed by one route trait to a controller.
///
/// A controller can implement multiple route contracts. The descriptor keeps
/// each contract separate so callers can preserve trait and method order while
/// validating the combined controller surface.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RouteContractDescriptor {
    trait_name: &'static str,
    routes: &'static [RouteDescriptor],
}

impl RouteContractDescriptor {
    /// Creates route-contract metadata emitted by `#[controller]`.
    ///
    /// The route slice must remain available for the entire program because
    /// descriptors are registered as immutable static metadata. Runtime
    /// validation rejects an empty trait name, an empty route slice, and
    /// duplicate contract declarations.
    ///
    /// # Examples
    ///
    /// ```
    /// use mads_common::{HttpMethod, RouteContractDescriptor, RouteDescriptor};
    /// use mads_common::core::SourceLocation;
    ///
    /// const ROUTES: &[RouteDescriptor] = &[RouteDescriptor::new(
    ///     HttpMethod::Get,
    ///     "",
    ///     "/health",
    ///     "/health",
    ///     "health",
    ///     SourceLocation::new("routes.rs", 1, 1),
    /// )];
    /// const CONTRACT: RouteContractDescriptor =
    ///     RouteContractDescriptor::new("HealthRoutes", ROUTES);
    ///
    /// assert_eq!(CONTRACT.trait_name(), "HealthRoutes");
    /// ```
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
///
/// Values are collected through the internal inventory registry when the
/// application is linked. The descriptor stores no controller instance and is
/// therefore safe to inspect during bootstrap before dependency construction.
pub struct ControllerRouteDescriptor {
    type_name: &'static str,
    type_id: fn() -> TypeId,
    namespace: Option<&'static str>,
    contracts: &'static [RouteContractDescriptor],
    registrar: Option<ControllerRegistrar>,
}

impl ControllerRouteDescriptor {
    /// Creates static controller-route metadata emitted by `#[controller]`.
    ///
    /// This constructor intentionally creates metadata without executable
    /// registrar code. It is useful for catalog inspection and compatibility
    /// tooling, but HTTP bootstrap rejects it with `MADS030`; use
    /// [`Self::with_registrar`] for a controller that can be installed in a
    /// router.
    pub const fn new(
        type_name: &'static str,
        type_id: fn() -> TypeId,
        contracts: &'static [RouteContractDescriptor],
    ) -> Self {
        Self {
            type_name,
            type_id,
            namespace: None,
            contracts,
            registrar: None,
        }
    }

    /// Creates controller metadata with its typed HTTP route registrar.
    ///
    /// The registrar is invoked only after the complete descriptor catalog has
    /// passed validation. It must consume exactly the validated routes supplied
    /// by [`ValidatedRouteIter`], then return the updated router.
    pub const fn with_registrar(
        type_name: &'static str,
        type_id: fn() -> TypeId,
        contracts: &'static [RouteContractDescriptor],
        registrar: ControllerRegistrar,
    ) -> Self {
        Self {
            type_name,
            type_id,
            namespace: None,
            contracts,
            registrar: Some(registrar),
        }
    }

    /// Attaches the Rust namespace containing this controller declaration.
    #[doc(hidden)]
    #[must_use]
    pub const fn with_namespace(mut self, namespace: &'static str) -> Self {
        self.namespace = Some(namespace);
        self
    }

    /// Returns the Rust namespace containing this controller declaration, when available.
    #[doc(hidden)]
    pub const fn namespace(&self) -> Option<&'static str> {
        self.namespace
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

    fn registrar(&self) -> Option<ControllerRegistrar> {
        self.registrar
    }
}

/// Runtime state shared by generated controller and route registrars.
#[doc(hidden)]
pub struct RouterBuildContext<'a> {
    application: &'a mads_core::ApplicationContext,
    #[cfg(feature = "jwt")]
    passport: &'a PassportStrategyPreflight<'static>,
}

impl<'a> RouterBuildContext<'a> {
    pub(crate) const fn new(
        application: &'a mads_core::ApplicationContext,
        #[cfg(feature = "jwt")] passport: &'a PassportStrategyPreflight<'static>,
    ) -> Self {
        Self {
            application,
            #[cfg(feature = "jwt")]
            passport,
        }
    }

    /// Returns the completed application context for controller resolution.
    #[doc(hidden)]
    pub const fn application(&self) -> &'a mads_core::ApplicationContext {
        self.application
    }

    /// Builds middleware state from the binding selected for one guarded route.
    #[cfg(feature = "jwt")]
    #[doc(hidden)]
    #[allow(clippy::result_large_err)]
    pub fn passport_guard_state(
        &self,
        guard: &'static GuardDescriptor,
    ) -> Result<PassportGuardState> {
        let binding = self
            .passport
            .binding_for(guard)
            .ok_or_else(|| missing_scoped_binding_error(guard))?;
        Ok(PassportGuardState::from_binding(self.application, binding))
    }
}

#[cfg(feature = "jwt")]
fn missing_scoped_binding_error(guard: &GuardDescriptor) -> Error {
    Error::new(
        Diagnostic::new(
            crate::passport::MADS130,
            "missing scoped Passport binding",
            "the selected HTTP application has no preflight Passport strategy binding for this guard",
        )
        .with_subject(guard.requirement_subject())
        .with_location(guard.location())
        .with_suggestion("build the router from complete selected Passport guard metadata"),
    )
}

/// Registers the validated routes for one controller on an Axum router.
///
/// This function-pointer type is used by generated controller adapters. Normal
/// Applications should use [`crate::build_router`] instead of calling a
/// registrar directly.
pub type ControllerRegistrar = fn(
    axum::Router,
    &RouterBuildContext<'_>,
    &mut ValidatedRouteIter<'_>,
) -> mads_core::Result<axum::Router>;

/// A controller whose metadata has passed HTTP runtime validation.
///
/// This is implementation support for generated code and the HTTP adapter.
#[doc(hidden)]
#[derive(Debug)]
pub struct ValidatedController {
    registrar: ControllerRegistrar,
    routes: Vec<ValidatedRoute>,
}

impl ValidatedController {
    /// Returns the generated registrar for this controller.
    #[doc(hidden)]
    pub const fn registrar(&self) -> ControllerRegistrar {
        self.registrar
    }

    /// Returns the validated routes in declaration order.
    #[doc(hidden)]
    pub fn routes(&self) -> ValidatedRouteIter<'_> {
        ValidatedRouteIter {
            routes: self.routes.iter(),
        }
    }
}

#[derive(Debug)]
struct ValidatedRoute {
    method: HttpMethod,
    handler: &'static str,
    selected: bool,
    axum_path: Option<String>,
}

/// Iterates validated Axum paths for a generated controller registrar.
///
/// This is implementation support for procedural macro expansions and is not
/// a stable application-facing API.
#[doc(hidden)]
pub struct ValidatedRouteIter<'a> {
    routes: std::slice::Iter<'a, ValidatedRoute>,
}

impl<'a> ValidatedRouteIter<'a> {
    /// Returns the next selected Axum path when its metadata matches.
    #[allow(clippy::result_large_err)]
    #[doc(hidden)]
    pub fn next(&mut self, method: HttpMethod, handler: &str) -> Result<Option<&'a str>> {
        let Some(route) = self.routes.next() else {
            return Err(metadata_error(
                "route registration",
                format!(
                    "generated registrar requested unexpected route {} `{handler}`",
                    method.as_str()
                ),
                None,
            ));
        };
        if route.method != method || route.handler != handler {
            return Err(metadata_error(
                "route registration",
                format!(
                    "generated registrar requested {} `{handler}`, but validation supplied {} `{}`",
                    method.as_str(),
                    route.method.as_str(),
                    route.handler,
                ),
                None,
            ));
        }
        Ok(route.selected.then(|| {
            route
                .axum_path
                .as_deref()
                .expect("selected routes retain a validated Axum path")
        }))
    }

    /// Verifies that the generated registrar consumed every validated route.
    #[allow(clippy::result_large_err)]
    #[doc(hidden)]
    pub fn finish(&mut self) -> Result<()> {
        if let Some(route) = self.routes.next() {
            return Err(metadata_error(
                "route registration",
                format!(
                    "generated registrar did not register {} `{}`",
                    route.method.as_str(),
                    route.handler,
                ),
                None,
            ));
        }
        Ok(())
    }
}

mads_core::__private::inventory::collect!(ControllerRouteDescriptor);

/// Looks up and validates route-contract metadata.
///
/// The catalog is a read-only view of metadata emitted by `#[routes]` and
/// `#[controller]`. Validation canonicalizes parameter names, so routes such as
/// `/users/:id` and `/users/:user_id` conflict when they use the same HTTP
/// method and controller scope. A conflict is reported as the framework's
/// `MADS030` diagnostic.
pub struct RouteCatalog;

impl RouteCatalog {
    /// Returns registered controllers in deterministic type-name order.
    ///
    /// The returned vector contains references to static descriptors; cloning
    /// the vector does not clone controller state. This is an inspection-only
    /// operation and does not resolve providers, invoke registrars, or start
    /// lifecycle hooks.
    ///
    /// # Examples
    ///
    /// ```
    /// use mads_common::RouteCatalog;
    ///
    /// let controllers = RouteCatalog::controllers();
    /// assert!(controllers.iter().all(|controller| !controller.type_name().is_empty()));
    /// ```
    pub fn controllers() -> Vec<&'static ControllerRouteDescriptor> {
        controller_cache().clone()
    }

    /// Returns route descriptors declared by controller `T`, preserving trait
    /// and method order.
    ///
    /// An unregistered type produces an empty vector. `T` must be a concrete,
    /// thread-safe application type because it is matched by its `TypeId`.
    ///
    /// # Examples
    ///
    /// ```
    /// use mads_common::RouteCatalog;
    ///
    /// struct NotAController;
    /// assert!(RouteCatalog::routes_for::<NotAController>().is_empty());
    /// ```
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
    ///
    /// If `T` is not registered, validation succeeds with no work. Otherwise,
    /// duplicate method/canonical-path pairs return a diagnostic containing the
    /// current and first declarations.
    ///
    /// # Errors
    ///
    /// Returns a [`mads_core::Error`] with diagnostic code `MADS030` when the
    /// controller metadata is malformed or contains a conflicting route.
    ///
    /// # Examples
    ///
    /// ```
    /// use mads_common::RouteCatalog;
    ///
    /// struct NotAController;
    /// assert!(RouteCatalog::validate_controller::<NotAController>().is_ok());
    /// ```
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
    ///
    /// This is the application-wide validation entry point and should run
    /// before a runtime adapter installs HTTP routes.
    ///
    /// # Errors
    ///
    /// Returns a [`mads_core::Error`] with diagnostic code `MADS030` for the
    /// first invalid descriptor or duplicate route found in the deterministic
    /// catalog order.
    ///
    /// # Examples
    ///
    /// ```
    /// use mads_common::RouteCatalog;
    ///
    /// assert!(RouteCatalog::validate().is_ok());
    /// ```
    #[allow(clippy::result_large_err)]
    pub fn validate() -> Result<()> {
        let _ = Self::validated()?;
        Ok(())
    }

    /// Returns every controller after validating runtime route metadata.
    ///
    /// The returned controllers are deterministically ordered and expose only
    /// doc-hidden support used by the generated HTTP adapter.
    #[allow(clippy::result_large_err)]
    #[doc(hidden)]
    pub fn validated() -> Result<Vec<ValidatedController>> {
        let controllers = Self::controllers();
        validate_descriptors(&controllers)
    }

    /// Returns controllers selected for `application` after scoped route validation.
    #[allow(clippy::result_large_err)]
    #[doc(hidden)]
    pub fn validated_for(application: &mads_core::Mads) -> Result<Vec<ValidatedController>> {
        let scope = crate::http_scope::HttpApplicationScope::for_application(application)?;
        validate_scoped_descriptors(scope.controllers())
    }
}

/// Validates an explicit controller descriptor slice for generated adapters.
///
/// This accepts manually supplied metadata so integration crates receive the
/// same fail-closed validation as inventory-registered controllers.
#[allow(clippy::result_large_err)]
#[doc(hidden)]
pub fn validate_descriptors(
    descriptors: &[&ControllerRouteDescriptor],
) -> Result<Vec<ValidatedController>> {
    validate_with_selection(descriptors, |_, _| true)
}

pub(crate) fn validate_scoped_descriptors(
    descriptors: &[ScopedController],
) -> Result<Vec<ValidatedController>> {
    let controllers = descriptors
        .iter()
        .map(ScopedController::descriptor)
        .collect::<Vec<_>>();
    validate_with_selection(&controllers, |controller, route| {
        descriptors
            .iter()
            .find(|scoped| std::ptr::eq(scoped.descriptor(), controller))
            .is_some_and(|scoped| scoped.selects(route))
    })
}

fn validate_with_selection(
    descriptors: &[&ControllerRouteDescriptor],
    is_selected: impl Fn(&ControllerRouteDescriptor, &RouteDescriptor) -> bool,
) -> Result<Vec<ValidatedController>> {
    let mut controllers = descriptors.to_vec();
    controllers.sort_by(|left, right| controller_sort_key(left).cmp(&controller_sort_key(right)));

    let mut identities: Vec<&ControllerRouteDescriptor> = Vec::new();
    let mut seen_routes: BTreeMap<(HttpMethod, String), (&str, RouteDescriptor)> = BTreeMap::new();
    let mut validated = Vec::with_capacity(controllers.len());

    for controller in controllers {
        validate_controller_identity(controller, &identities)?;
        validate_contract(controller)?;
        let Some(registrar) = controller.registrar() else {
            return Err(metadata_error(
                controller.type_name(),
                "controller metadata does not include a typed HTTP registrar",
                controller_location(controller),
            ));
        };

        let mut routes = Vec::new();
        for contract in controller.contracts() {
            for route in contract.routes() {
                let selected = is_selected(controller, route);
                if selected {
                    validate_route(route)?;
                    let key = (route.method(), canonical_pattern(route.full_path()));
                    if let Some((previous_controller, previous_route)) =
                        seen_routes.insert(key, (controller.type_name(), *route))
                    {
                        return Err(conflicting_routes(
                            controller.type_name(),
                            *route,
                            previous_controller,
                            previous_route,
                            "application",
                        ));
                    }
                }
                routes.push(ValidatedRoute {
                    method: route.method(),
                    handler: route.handler(),
                    selected,
                    axum_path: selected.then(|| to_axum_path(route.full_path())),
                });
            }
        }

        identities.push(controller);
        validated.push(ValidatedController { registrar, routes });
    }

    Ok(validated)
}

fn controller_cache() -> &'static Vec<&'static ControllerRouteDescriptor> {
    static CONTROLLERS: OnceLock<Vec<&'static ControllerRouteDescriptor>> = OnceLock::new();
    CONTROLLERS.get_or_init(|| {
        let mut controllers: Vec<_> =
            mads_core::__private::inventory::iter::<ControllerRouteDescriptor>
                .into_iter()
                .collect();
        controllers.sort_by(|left, right| {
            left.type_name()
                .cmp(right.type_name())
                .then_with(|| left.namespace().cmp(&right.namespace()))
        });
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

fn validate_controller_identity(
    controller: &ControllerRouteDescriptor,
    previous: &[&ControllerRouteDescriptor],
) -> Result<()> {
    if controller.type_name().is_empty() {
        return Err(metadata_error(
            "controller identity",
            "controller type name must not be empty",
            controller_location(controller),
        ));
    }

    for earlier in previous {
        if earlier.type_id() == controller.type_id() {
            return Err(duplicate_controller_identity(
                "controller type identifier",
                controller,
                earlier,
            ));
        }
        if earlier.type_name() == controller.type_name() {
            return Err(duplicate_controller_identity(
                "controller type name",
                controller,
                earlier,
            ));
        }
    }
    Ok(())
}

fn validate_contract(controller: &ControllerRouteDescriptor) -> Result<()> {
    if controller.contracts().is_empty() {
        return Err(metadata_error(
            controller.type_name(),
            "controller must declare at least one route contract",
            controller_location(controller),
        ));
    }

    let mut names = BTreeMap::new();
    for contract in controller.contracts() {
        if contract.trait_name().is_empty() {
            return Err(metadata_error(
                controller.type_name(),
                "route contract name must not be empty",
                controller_location(controller),
            ));
        }
        if contract.routes().is_empty() {
            return Err(metadata_error(
                contract.trait_name(),
                "route contract must declare at least one active route",
                controller_location(controller),
            ));
        }
        if names.insert(contract.trait_name(), ()).is_some() {
            return Err(metadata_error(
                contract.trait_name(),
                "controller contains a duplicate route contract descriptor",
                controller_location(controller),
            ));
        }
    }
    Ok(())
}

fn validate_route(route: &RouteDescriptor) -> Result<()> {
    validate_path(route.prefix(), true, "route prefix", route.location())?;
    validate_path(route.path(), false, "route path", route.location())?;
    validate_path(
        route.full_path(),
        false,
        "route full path",
        route.location(),
    )?;
    if canonical_join(route.prefix(), route.path()) != route.full_path() {
        return Err(metadata_error(
            format!("{} {}", route.method().as_str(), route.full_path()),
            "route full path does not match the canonical prefix and endpoint join",
            Some(route.location()),
        ));
    }
    validate_source_location(route.location())
}

fn canonical_join(prefix: &str, path: &str) -> String {
    if prefix.is_empty() || prefix == "/" {
        path.to_owned()
    } else if path == "/" {
        prefix.to_owned()
    } else {
        format!("{prefix}{path}")
    }
}

fn validate_path(
    value: &str,
    is_prefix: bool,
    subject: &str,
    location: SourceLocation,
) -> Result<()> {
    if is_prefix && value.is_empty() {
        return Ok(());
    }
    if value.is_empty() || !value.starts_with('/') {
        return Err(metadata_error(
            subject,
            format!("{subject} must be non-empty and start with `/`"),
            Some(location),
        ));
    }
    if value.contains(['?', '#']) {
        return Err(metadata_error(
            subject,
            format!("{subject} must not contain a query string or fragment"),
            Some(location),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(metadata_error(
            subject,
            format!("{subject} must not contain control characters"),
            Some(location),
        ));
    }
    if value.contains(['\\', '%']) || value.chars().any(char::is_whitespace) {
        return Err(metadata_error(
            subject,
            format!("{subject} must not contain backslashes, percent-encoding, or whitespace"),
            Some(location),
        ));
    }
    if value != "/" && value.ends_with('/') {
        return Err(metadata_error(
            subject,
            format!("{subject} must not end with `/`; use `/` only for the root route"),
            Some(location),
        ));
    }
    if value == "/" {
        return Ok(());
    }

    let mut parameters = BTreeMap::new();
    for segment in value.split('/').skip(1) {
        if segment.is_empty() || matches!(segment, "." | "..") {
            return Err(metadata_error(
                subject,
                format!("{subject} must not contain empty, `.` or `..` segments"),
                Some(location),
            ));
        }
        if segment.starts_with('*') || segment.contains(['{', '}']) {
            return Err(metadata_error(
                subject,
                format!(
                    "{subject} must not use Axum wildcard or brace-capture syntax; use `:parameter` captures"
                ),
                Some(location),
            ));
        }
        if let Some(parameter) = segment.strip_prefix(':') {
            let mut characters = parameter.chars();
            let Some(first) = characters.next() else {
                return Err(metadata_error(
                    subject,
                    format!("{subject} contains an empty parameter"),
                    Some(location),
                ));
            };
            if !(first == '_' || first.is_ascii_alphabetic())
                || !characters
                    .all(|character| character == '_' || character.is_ascii_alphanumeric())
            {
                return Err(metadata_error(
                    subject,
                    format!("{subject} parameters must use `[A-Za-z_][A-Za-z0-9_]*`"),
                    Some(location),
                ));
            }
            if parameters.insert(parameter, ()).is_some() {
                return Err(metadata_error(
                    subject,
                    format!("{subject} must not repeat parameter `:{parameter}`"),
                    Some(location),
                ));
            }
        } else if segment.contains(':') {
            return Err(metadata_error(
                subject,
                format!("{subject} parameters must occupy an entire path segment"),
                Some(location),
            ));
        }
    }

    if is_prefix && value.contains(':') {
        return Err(metadata_error(
            subject,
            "route prefix must not contain parameters; declare them on the endpoint path",
            Some(location),
        ));
    }
    Ok(())
}

fn validate_source_location(location: SourceLocation) -> Result<()> {
    if location.file.is_empty() || location.line == 0 || location.column == 0 {
        return Err(metadata_error(
            "route source location",
            "route source file, line, and column must be present",
            None,
        ));
    }
    Ok(())
}

fn validate_routes(
    scope: &'static str,
    routes: impl IntoIterator<Item = (&'static str, RouteDescriptor)>,
) -> Result<()> {
    let mut seen = BTreeMap::new();
    for (controller, route) in routes {
        let key = (route.method(), canonical_pattern(route.full_path()));
        if let Some((previous_controller, previous_route)) = seen.insert(key, (controller, route)) {
            return Err(conflicting_routes(
                controller,
                route,
                previous_controller,
                previous_route,
                scope,
            ));
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

fn to_axum_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            segment.strip_prefix(':').map_or_else(
                || segment.to_owned(),
                |parameter| format!("{{{parameter}}}"),
            )
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn controller_sort_key(
    controller: &ControllerRouteDescriptor,
) -> (&'static str, Option<&'static str>, &'static str, u32, u32) {
    let location = controller_location(controller).unwrap_or(SourceLocation::new("", 0, 0));
    (
        controller.type_name(),
        controller.namespace(),
        location.file,
        location.line,
        location.column,
    )
}

fn controller_location(controller: &ControllerRouteDescriptor) -> Option<SourceLocation> {
    controller
        .contracts()
        .iter()
        .find_map(|contract| contract.routes().first().map(|route| route.location()))
}

fn duplicate_controller_identity(
    subject: &str,
    current: &ControllerRouteDescriptor,
    previous: &ControllerRouteDescriptor,
) -> Error {
    let primary = metadata_diagnostic(
        subject,
        format!(
            "`{}` conflicts with previously registered controller `{}`",
            current.type_name(),
            previous.type_name(),
        ),
        controller_location(current),
    );
    let related = metadata_diagnostic(
        "first controller declaration",
        "the conflicting controller was first declared here",
        controller_location(previous),
    );
    Error::from_diagnostics(primary, [related])
}

fn conflicting_routes(
    controller: &str,
    route: RouteDescriptor,
    previous_controller: &str,
    previous_route: RouteDescriptor,
    scope: &str,
) -> Error {
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
    Error::from_diagnostics(primary, [related])
}

fn metadata_error(
    subject: impl Into<String>,
    message: impl Into<String>,
    location: Option<SourceLocation>,
) -> Error {
    Error::new(metadata_diagnostic(subject, message, location))
}

fn metadata_diagnostic(
    subject: impl Into<String>,
    message: impl Into<String>,
    location: Option<SourceLocation>,
) -> Diagnostic {
    let diagnostic = Diagnostic::new(MADS030, "invalid route metadata", message)
        .with_subject(subject)
        .with_suggestion("correct the route metadata before starting the HTTP runtime");
    if let Some(location) = location {
        diagnostic.with_location(location)
    } else {
        diagnostic
    }
}

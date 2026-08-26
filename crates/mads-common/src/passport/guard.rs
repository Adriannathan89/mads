//! Static Passport guard policy metadata.
//!
//! Route macros emit immutable descriptors here. Router construction resolves
//! each descriptor to one strategy adapter, and the route middleware below
//! uses that same binding to authenticate and authorize requests.

use std::any::TypeId;
use std::cmp::Ordering;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::{Context, Poll};

use axum::{
    extract::{Request, connect_info::ConnectInfo},
    http::header::AUTHORIZATION,
    response::{IntoResponse, Response},
};
use mads_core::{Diagnostic, Error, Result, SourceLocation};
use tower::{Layer, Service};

use super::{
    ErasedAuthentication, PassportContext, PassportError, PassportResult, PassportStrategyAdapter,
    PassportStrategyCatalog, PassportStrategyFuture,
};

/// The one token source selected by a Passport guard.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum TokenSource {
    /// Read one RFC 6750 Bearer credential from the `Authorization` header.
    Bearer,
    /// Read one strict request cookie by its literal name.
    Cookie(&'static str),
}

impl fmt::Debug for TokenSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bearer => formatter.write_str("Bearer"),
            // Cookie names can identify a deployment's authentication surface;
            // diagnostics intentionally retain only the source category.
            Self::Cookie(_) => formatter.write_str("Cookie(..)"),
        }
    }
}

/// The matching rule applied to a role or permission collection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PolicyMode {
    /// At least one configured value must match.
    Any,
    /// Every configured value must match.
    All,
}

/// One role or permission policy clause emitted by `#[guard]`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PolicyClause {
    mode: PolicyMode,
    values: &'static [&'static str],
}

impl PolicyClause {
    /// Creates one static policy clause.
    #[must_use]
    pub const fn new(mode: PolicyMode, values: &'static [&'static str]) -> Self {
        Self { mode, values }
    }

    /// Returns whether this clause uses `any` or `all` matching.
    #[must_use]
    pub const fn mode(self) -> PolicyMode {
        self.mode
    }

    /// Returns the configured role or permission values.
    #[must_use]
    pub const fn values(self) -> &'static [&'static str] {
        self.values
    }
}

/// A generated, type-safe policy predicate over an erased authentication.
pub type GuardPredicateAdapter = fn(&ErasedAuthentication) -> bool;

/// One named predicate emitted by `#[guard]`.
#[derive(Clone, Copy)]
pub struct GuardPredicate {
    name: &'static str,
    adapter: Option<GuardPredicateAdapter>,
}

impl GuardPredicate {
    /// Creates static predicate metadata.
    ///
    /// `None` is accepted solely so manually supplied metadata can be
    /// validated fail-closed by [`GuardCatalog::validate`]. Macro-generated
    /// descriptors always provide an adapter.
    #[must_use]
    pub const fn new(name: &'static str, adapter: Option<GuardPredicateAdapter>) -> Self {
        Self { name, adapter }
    }

    /// Returns the source path recorded for this predicate.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the generated type-safe adapter when metadata is valid.
    #[doc(hidden)]
    #[must_use]
    pub const fn adapter(self) -> Option<GuardPredicateAdapter> {
        self.adapter
    }
}

/// Framework adapter for the built-in `jwt` strategy supplied by a direct
/// `ClaimsPrincipal<C>` guard.
pub type BuiltinGuardAdapter = for<'a> fn(
    &'a mads_core::ApplicationContext,
    &'a PassportContext<'a>,
    &'a str,
) -> PassportStrategyFuture<'a>;

/// Immutable effective policy for one guarded route method.
pub struct GuardDescriptor {
    route_trait: &'static str,
    handler: &'static str,
    requirement_subject: &'static str,
    strategy: &'static str,
    principal_type_id: Option<fn() -> TypeId>,
    principal_type_name: Option<fn() -> &'static str>,
    source: TokenSource,
    roles: Option<PolicyClause>,
    permissions: Option<PolicyClause>,
    predicates: &'static [GuardPredicate],
    location: SourceLocation,
    builtin_adapter: Option<BuiltinGuardAdapter>,
}

impl GuardDescriptor {
    /// Creates static effective guard metadata.
    ///
    /// The optional principal factories permit integrations to build
    /// descriptors manually. [`GuardCatalog::validate`] rejects omitted or
    /// inconsistent factories before startup.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        route_trait: &'static str,
        handler: &'static str,
        strategy: &'static str,
        principal_type_id: Option<fn() -> TypeId>,
        principal_type_name: Option<fn() -> &'static str>,
        source: TokenSource,
        roles: Option<PolicyClause>,
        permissions: Option<PolicyClause>,
        predicates: &'static [GuardPredicate],
        location: SourceLocation,
        builtin_adapter: Option<BuiltinGuardAdapter>,
    ) -> Self {
        Self {
            route_trait,
            handler,
            requirement_subject: "manual Passport guard",
            strategy,
            principal_type_id,
            principal_type_name,
            source,
            roles,
            permissions,
            predicates,
            location,
            builtin_adapter,
        }
    }

    /// Attaches the stable requirement subject emitted by `#[guard]`.
    ///
    /// Route expansion supplies `RouteTrait::method` with `concat!`, keeping
    /// auto-configuration evidence static and free of application values.
    #[doc(hidden)]
    #[must_use]
    pub const fn with_requirement_subject(mut self, subject: &'static str) -> Self {
        self.requirement_subject = subject;
        self
    }

    /// Returns the route-contract trait name.
    #[must_use]
    pub const fn route_trait(&self) -> &'static str {
        self.route_trait
    }

    /// Returns the guarded route method name.
    #[must_use]
    pub const fn handler(&self) -> &'static str {
        self.handler
    }

    /// Returns the stable subject used for JWT auto-configuration evidence.
    #[doc(hidden)]
    #[must_use]
    pub const fn requirement_subject(&self) -> &'static str {
        self.requirement_subject
    }

    /// Returns the requested Passport strategy name.
    #[must_use]
    pub const fn strategy(&self) -> &'static str {
        self.strategy
    }

    /// Returns the exact principal type identifier when present.
    #[must_use]
    pub fn principal_type_id(&self) -> Option<TypeId> {
        self.principal_type_id.map(|factory| factory())
    }

    /// Returns the exact principal type name when present.
    #[must_use]
    pub fn principal_type_name(&self) -> Option<&'static str> {
        self.principal_type_name.map(|factory| factory())
    }

    /// Returns the one token source used for this guard.
    #[must_use]
    pub const fn source(&self) -> TokenSource {
        self.source
    }

    /// Returns the optional roles clause.
    #[must_use]
    pub const fn roles(&self) -> Option<PolicyClause> {
        self.roles
    }

    /// Returns the optional permissions clause.
    #[must_use]
    pub const fn permissions(&self) -> Option<PolicyClause> {
        self.permissions
    }

    /// Returns every ANDed custom predicate in declaration order.
    #[must_use]
    pub const fn predicates(&self) -> &'static [GuardPredicate] {
        self.predicates
    }

    /// Returns the route declaration source location.
    #[must_use]
    pub const fn location(&self) -> SourceLocation {
        self.location
    }

    /// Returns a built-in typed-claims adapter when this guard can use `jwt`
    /// without a custom managed strategy.
    #[doc(hidden)]
    #[must_use]
    pub const fn builtin_adapter(&self) -> Option<BuiltinGuardAdapter> {
        self.builtin_adapter
    }
}

mads_core::__private::inventory::collect!(&'static GuardDescriptor);

/// Read-only inspection and validation of linked Passport route guards.
pub struct GuardCatalog;

impl GuardCatalog {
    /// Returns every linked effective guard in deterministic route order.
    #[must_use]
    pub fn guards() -> Vec<&'static GuardDescriptor> {
        guard_cache().clone()
    }

    /// Validates every linked guard descriptor before strategy resolution.
    ///
    /// # Errors
    ///
    /// Returns `MADS131` when any descriptor has incomplete or malformed
    /// static policy metadata.
    #[allow(clippy::result_large_err)]
    pub fn validate() -> Result<()> {
        Self::validate_descriptors(&Self::guards())
    }

    /// Validates an explicit static descriptor slice without registering it.
    ///
    /// This permits integration crates to validate manually assembled
    /// descriptors using the same fail-closed rules as inventory metadata.
    #[doc(hidden)]
    #[allow(clippy::result_large_err)]
    pub fn validate_descriptors(descriptors: &[&GuardDescriptor]) -> Result<()> {
        for guard in descriptors {
            validate_guard(guard)?;
        }
        Ok(())
    }
}

/// Per-route middleware state produced from one preflight-selected Passport guard.
///
/// Generated route registration constructs this state while the router is built,
/// so the request path uses the exact static descriptor and selected adapter
/// already validated for that route.
#[doc(hidden)]
#[derive(Clone)]
pub struct PassportGuardState {
    application: mads_core::ApplicationContext,
    guard: &'static GuardDescriptor,
    adapter: PassportStrategyAdapter,
}

impl PassportGuardState {
    /// Selects the one strategy adapter for a static guard descriptor.
    #[doc(hidden)]
    #[allow(clippy::result_large_err)]
    pub fn new(
        application: &mads_core::ApplicationContext,
        guard: &'static GuardDescriptor,
    ) -> Result<Self> {
        let guards = [guard];
        let preflight = PassportStrategyCatalog::preflight(&guards)?;
        let binding = preflight
            .binding_for(guard)
            .expect("the preflight result must retain its requested guard");
        Ok(Self {
            application: application.clone(),
            guard,
            adapter: binding.adapter(),
        })
    }
}

/// A route-specific Tower layer that executes a selected Passport guard.
#[doc(hidden)]
#[derive(Clone)]
pub struct PassportGuardLayer {
    state: PassportGuardState,
}

impl PassportGuardLayer {
    /// Creates one route-specific Passport middleware layer.
    #[doc(hidden)]
    #[must_use]
    pub const fn new(state: PassportGuardState) -> Self {
        Self { state }
    }
}

impl<S> Layer<S> for PassportGuardLayer {
    type Service = PassportGuardService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        PassportGuardService {
            inner,
            state: self.state.clone(),
        }
    }
}

/// A Tower service produced by [`PassportGuardLayer`].
#[doc(hidden)]
#[derive(Clone)]
pub struct PassportGuardService<S> {
    inner: S,
    state: PassportGuardState,
}

impl<S> Service<Request> for PassportGuardService<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Error: Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = std::result::Result<Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<std::result::Result<(), S::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, mut request: Request) -> Self::Future {
        let state = self.state.clone();
        let not_ready_inner = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, not_ready_inner);
        Box::pin(async move {
            if let Err(error) = state.authorize(&mut request).await {
                return Ok(super::PassportRejection::from(error).into_response());
            }
            inner.call(request).await
        })
    }
}

impl PassportGuardState {
    async fn authorize(&self, request: &mut Request) -> PassportResult<()> {
        let TokenSource::Bearer = self.guard.source() else {
            return Err(PassportError::reject());
        };
        let token = bearer_token(request.headers())?.to_owned();
        let headers = request.headers().clone();
        let method = request.method().clone();
        let uri = request.uri().clone();
        let remote_addr = request
            .extensions()
            .get::<ConnectInfo<std::net::SocketAddr>>()
            .map(|address| address.0);
        let context = PassportContext::new(&headers, &method, &uri, remote_addr);
        let authentication = (self.adapter)(&self.application, &context, &token).await?;
        if !authorizes(self.guard, &authentication) {
            return Err(PassportError::forbidden());
        }
        if !authentication.install_extensions(request.extensions_mut()) {
            return Err(PassportError::internal(std::io::Error::other(
                "guard authentication type binding failed",
            )));
        }
        Ok(())
    }
}

fn bearer_token(headers: &axum::http::HeaderMap) -> PassportResult<&str> {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let Some(value) = values.next() else {
        return Err(PassportError::reject());
    };
    if values.next().is_some() {
        return Err(PassportError::reject());
    }
    let value = value.to_str().map_err(|_| PassportError::reject())?;
    let mut parts = value.split_whitespace();
    let Some(scheme) = parts.next() else {
        return Err(PassportError::reject());
    };
    let Some(token) = parts.next() else {
        return Err(PassportError::reject());
    };
    if !scheme.eq_ignore_ascii_case("Bearer") || token.is_empty() || parts.next().is_some() {
        return Err(PassportError::reject());
    }
    Ok(token)
}

fn authorizes(guard: &GuardDescriptor, authentication: &ErasedAuthentication) -> bool {
    let principal = authentication.principal();
    if !matches_clause(guard.roles(), |value| principal.has_role(value))
        || !matches_clause(guard.permissions(), |value| principal.has_permission(value))
    {
        return false;
    }
    guard.predicates().iter().all(|predicate| {
        predicate
            .adapter()
            .is_some_and(|adapter| adapter(authentication))
    })
}

fn matches_clause(clause: Option<PolicyClause>, matches: impl FnMut(&str) -> bool) -> bool {
    let Some(clause) = clause else {
        return true;
    };
    match clause.mode() {
        PolicyMode::Any => clause.values().iter().copied().any(matches),
        PolicyMode::All => clause.values().iter().copied().all(matches),
    }
}

fn guard_cache() -> &'static Vec<&'static GuardDescriptor> {
    static CACHE: OnceLock<Vec<&'static GuardDescriptor>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut guards: Vec<_> = mads_core::__private::inventory::iter::<&'static GuardDescriptor>
            .into_iter()
            .copied()
            .collect();
        guards.sort_by(guard_order);
        guards
    })
}

fn guard_order(left: &&'static GuardDescriptor, right: &&'static GuardDescriptor) -> Ordering {
    left.route_trait()
        .cmp(right.route_trait())
        .then_with(|| left.handler().cmp(right.handler()))
        .then_with(|| location_order(left.location(), right.location()))
}

fn location_order(left: SourceLocation, right: SourceLocation) -> Ordering {
    left.file
        .cmp(right.file)
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.column.cmp(&right.column))
}

fn validate_guard(guard: &GuardDescriptor) -> Result<()> {
    let subject = guard_subject(guard);
    if guard.route_trait().is_empty() || guard.handler().is_empty() {
        return Err(metadata_error(
            subject,
            "route trait and handler names must be present",
            guard.location(),
        ));
    }
    if !valid_strategy_name(guard.strategy()) {
        return Err(metadata_error(
            subject,
            "guard strategy name must be non-empty lowercase ASCII `[a-z0-9._-]`",
            guard.location(),
        ));
    }
    if guard.principal_type_id().is_none() || guard.principal_type_name().is_none_or(str::is_empty)
    {
        return Err(metadata_error(
            subject,
            "guard principal type metadata must be present",
            guard.location(),
        ));
    }
    if let TokenSource::Cookie(name) = guard.source()
        && !valid_cookie_name(name)
    {
        return Err(metadata_error(
            subject,
            "guard cookie token source must use a non-empty RFC cookie name",
            guard.location(),
        ));
    }
    for (label, clause) in [
        ("roles", guard.roles()),
        ("permissions", guard.permissions()),
    ] {
        if let Some(clause) = clause
            && (clause.values().is_empty()
                || clause
                    .values()
                    .iter()
                    .any(|value| value.is_empty() || value.chars().any(char::is_control)))
        {
            return Err(metadata_error(
                subject,
                format!("guard {label} policy must contain non-empty values"),
                guard.location(),
            ));
        }
    }
    if guard
        .predicates()
        .iter()
        .any(|predicate| predicate.name().is_empty() || predicate.adapter().is_none())
    {
        return Err(metadata_error(
            subject,
            "guard predicates must have a name and generated adapter",
            guard.location(),
        ));
    }
    if guard.location().file.is_empty()
        || guard.location().line == 0
        || guard.location().column == 0
    {
        return Err(metadata_error(
            subject,
            "guard source file, line, and column must be present",
            guard.location(),
        ));
    }
    Ok(())
}

fn guard_subject(guard: &GuardDescriptor) -> String {
    format!("{}::{}", guard.route_trait(), guard.handler())
}

fn valid_strategy_name(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn valid_cookie_name(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

fn metadata_error(subject: String, message: impl Into<String>, location: SourceLocation) -> Error {
    Error::new(
        Diagnostic::new(super::MADS131, "invalid Passport guard metadata", message)
            .with_subject(subject)
            .with_location(location)
            .with_suggestion("correct the guard metadata before starting the application"),
    )
}

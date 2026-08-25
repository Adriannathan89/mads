//! Managed Passport strategy metadata and verified-claims adapters.

use std::any::{Any, TypeId, type_name};
use std::cmp::Ordering;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use mads_core::{ApplicationContext, SourceLocation};

use crate::{JwtClaims, JwtTokenKind, VerifiedJwt};

use super::{PassportContext, PassportPrincipal, PassportResult};

/// A typed application strategy that turns verified JWT claims into a principal.
///
/// Passport always verifies JWT cryptography, registered claims, and token kind
/// before invoking this application-owned validation method. Implementations are
/// registered only when annotated with [`crate::passport_strategy`].
#[allow(async_fn_in_trait)]
pub trait PassportStrategy: Send + Sync + 'static {
    /// The application-owned custom JWT claims consumed by this strategy.
    type Claims: serde::de::DeserializeOwned + Send + Sync + 'static;
    /// The authenticated application principal returned by this strategy.
    type Principal: PassportPrincipal;

    /// The MADS access or refresh token profile accepted by this strategy.
    const TOKEN_KIND: JwtTokenKind;

    /// Validates verified custom claims into an authenticated principal.
    async fn validate(
        &self,
        context: &PassportContext<'_>,
        claims: &JwtClaims<Self::Claims>,
    ) -> PassportResult<Self::Principal>;
}

/// The future returned by an erased Passport strategy adapter.
pub type PassportStrategyFuture<'a> =
    Pin<Box<dyn Future<Output = PassportResult<ErasedAuthentication>> + Send + 'a>>;

/// Invokes one concrete Passport strategy after framework JWT verification.
///
/// This function-pointer type is emitted by [`crate::passport_strategy`].
/// Applications should select strategies through guards rather than call an
/// adapter directly. The raw token argument is retained exclusively for
/// framework verification; application validation receives only the verified
/// claims and the credential-sanitized [`PassportContext`].
pub type PassportStrategyAdapter = for<'a> fn(
    &'a ApplicationContext,
    &'a PassportContext<'a>,
    &'a str,
) -> PassportStrategyFuture<'a>;

/// The type-erased result of a successful Passport strategy validation.
///
/// The record preserves the policy-capable principal and exact typed principal
/// and verified-JWT allocations so the guard runtime can install typed request
/// extensions without re-validating claims.
pub struct ErasedAuthentication {
    principal: Arc<dyn PassportPrincipal>,
    exact_principal: Arc<dyn Any + Send + Sync>,
    exact_verified: Arc<dyn Any + Send + Sync>,
    principal_type_id: TypeId,
    principal_type_name: &'static str,
    claims_type_id: TypeId,
    claims_type_name: &'static str,
}

impl ErasedAuthentication {
    /// Creates a type-erased authentication record for macro-generated adapters.
    #[doc(hidden)]
    pub fn new<P, C>(principal: P, verified: VerifiedJwt<C>) -> Self
    where
        P: PassportPrincipal,
        C: Send + Sync + 'static,
    {
        let exact_principal = Arc::new(principal);
        let policy_principal: Arc<dyn PassportPrincipal> = exact_principal.clone();
        let exact_verified = Arc::new(verified);
        Self {
            principal: policy_principal,
            exact_principal,
            exact_verified,
            principal_type_id: TypeId::of::<P>(),
            principal_type_name: type_name::<P>(),
            claims_type_id: TypeId::of::<C>(),
            claims_type_name: type_name::<C>(),
        }
    }

    /// Returns the policy-capable application principal.
    #[must_use]
    pub fn principal(&self) -> &(dyn PassportPrincipal + 'static) {
        self.principal.as_ref()
    }

    /// Returns the exact principal type identifier.
    #[must_use]
    pub const fn principal_type_id(&self) -> TypeId {
        self.principal_type_id
    }

    /// Returns the exact principal type name.
    #[must_use]
    pub const fn principal_type_name(&self) -> &'static str {
        self.principal_type_name
    }

    /// Returns the exact verified custom-claims type identifier.
    #[must_use]
    pub const fn claims_type_id(&self) -> TypeId {
        self.claims_type_id
    }

    /// Returns the exact verified custom-claims type name.
    #[must_use]
    pub const fn claims_type_name(&self) -> &'static str {
        self.claims_type_name
    }

    /// Returns the exact principal allocation when it has type `P`.
    #[doc(hidden)]
    pub fn principal_as<P>(&self) -> Option<Arc<P>>
    where
        P: PassportPrincipal,
    {
        Arc::downcast::<P>(Arc::clone(&self.exact_principal)).ok()
    }

    /// Returns the exact verified-JWT allocation when its custom claims are `C`.
    #[doc(hidden)]
    pub fn verified_as<C>(&self) -> Option<Arc<VerifiedJwt<C>>>
    where
        C: Send + Sync + 'static,
    {
        Arc::downcast::<VerifiedJwt<C>>(Arc::clone(&self.exact_verified)).ok()
    }
}

impl fmt::Debug for ErasedAuthentication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ErasedAuthentication")
            .field("principal_type", &self.principal_type_name)
            .field("claims_type", &self.claims_type_name)
            .finish_non_exhaustive()
    }
}

/// Static metadata emitted for one managed Passport strategy implementation.
pub struct PassportStrategyDescriptor {
    name: &'static str,
    provider_type_id: fn() -> TypeId,
    provider_type_name: fn() -> &'static str,
    claims_type_id: fn() -> TypeId,
    claims_type_name: fn() -> &'static str,
    principal_type_id: fn() -> TypeId,
    principal_type_name: fn() -> &'static str,
    token_kind: JwtTokenKind,
    location: SourceLocation,
    adapter: PassportStrategyAdapter,
}

impl PassportStrategyDescriptor {
    /// Creates metadata emitted by [`crate::passport_strategy`].
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        name: &'static str,
        provider_type_id: fn() -> TypeId,
        provider_type_name: fn() -> &'static str,
        claims_type_id: fn() -> TypeId,
        claims_type_name: fn() -> &'static str,
        principal_type_id: fn() -> TypeId,
        principal_type_name: fn() -> &'static str,
        token_kind: JwtTokenKind,
        location: SourceLocation,
        adapter: PassportStrategyAdapter,
    ) -> Self {
        Self {
            name,
            provider_type_id,
            provider_type_name,
            claims_type_id,
            claims_type_name,
            principal_type_id,
            principal_type_name,
            token_kind,
            location,
            adapter,
        }
    }

    /// Returns the configured strategy name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the concrete managed provider type identifier.
    #[must_use]
    pub fn provider_type_id(&self) -> TypeId {
        (self.provider_type_id)()
    }

    /// Returns the concrete managed provider type name.
    #[must_use]
    pub fn provider_type_name(&self) -> &'static str {
        (self.provider_type_name)()
    }

    /// Returns the custom JWT claims type identifier.
    #[must_use]
    pub fn claims_type_id(&self) -> TypeId {
        (self.claims_type_id)()
    }

    /// Returns the custom JWT claims type name.
    #[must_use]
    pub fn claims_type_name(&self) -> &'static str {
        (self.claims_type_name)()
    }

    /// Returns the authenticated principal type identifier.
    #[must_use]
    pub fn principal_type_id(&self) -> TypeId {
        (self.principal_type_id)()
    }

    /// Returns the authenticated principal type name.
    #[must_use]
    pub fn principal_type_name(&self) -> &'static str {
        (self.principal_type_name)()
    }

    /// Returns the token profile required before this strategy runs.
    #[must_use]
    pub const fn token_kind(&self) -> JwtTokenKind {
        self.token_kind
    }

    /// Returns the strategy declaration source location.
    #[must_use]
    pub const fn location(&self) -> SourceLocation {
        self.location
    }

    /// Returns the generated framework adapter.
    #[doc(hidden)]
    #[must_use]
    pub const fn adapter(&self) -> PassportStrategyAdapter {
        self.adapter
    }
}

impl fmt::Debug for PassportStrategyDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PassportStrategyDescriptor")
            .field("name", &self.name)
            .field("provider_type", &self.provider_type_name())
            .field("claims_type", &self.claims_type_name())
            .field("principal_type", &self.principal_type_name())
            .field("token_kind", &self.token_kind)
            .field("location", &self.location)
            .finish_non_exhaustive()
    }
}

mads_core::__private::inventory::collect!(PassportStrategyDescriptor);

/// Read-only, deterministic inspection of managed Passport strategies.
pub struct PassportStrategyCatalog;

impl PassportStrategyCatalog {
    /// Returns every linked strategy descriptor in deterministic order.
    ///
    /// Descriptors are sorted by strategy name, provider type name, and source
    /// location. Inspection does not resolve providers or invoke strategies.
    #[must_use]
    pub fn strategies() -> Vec<&'static PassportStrategyDescriptor> {
        strategy_cache().clone()
    }
}

fn strategy_cache() -> &'static Vec<&'static PassportStrategyDescriptor> {
    static CACHE: OnceLock<Vec<&'static PassportStrategyDescriptor>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut strategies: Vec<_> =
            mads_core::__private::inventory::iter::<PassportStrategyDescriptor>
                .into_iter()
                .collect();
        strategies.sort_by(strategy_order);
        strategies
    })
}

fn strategy_order(
    left: &&'static PassportStrategyDescriptor,
    right: &&'static PassportStrategyDescriptor,
) -> Ordering {
    left.name()
        .cmp(right.name())
        .then_with(|| left.provider_type_name().cmp(right.provider_type_name()))
        .then_with(|| location_order(left.location(), right.location()))
}

fn location_order(left: SourceLocation, right: SourceLocation) -> Ordering {
    left.file
        .cmp(right.file)
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.column.cmp(&right.column))
}

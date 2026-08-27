//! Principal policy and guarded-request extractor contracts.

use std::{any::type_name, fmt, ops::Deref, sync::Arc};

use axum::{extract::FromRequestParts, http::request::Parts};

use crate::VerifiedJwt;

use super::{PassportError, PassportRejection};

/// An authenticated application identity that can answer guard policies.
pub trait PassportPrincipal: Send + Sync + 'static {
    /// Returns whether this principal has `role`.
    fn has_role(&self, role: &str) -> bool;

    /// Returns whether this principal has `permission`.
    fn has_permission(&self, permission: &str) -> bool;
}

/// A principal backed directly by a complete verified JWT.
pub struct ClaimsPrincipal<C> {
    verified: Arc<VerifiedJwt<C>>,
}

impl<C> ClaimsPrincipal<C> {
    /// Creates a principal from one complete verified JWT.
    ///
    /// This is used by the generated built-in `jwt` guard adapter. Application
    /// code normally receives this type through [`Authenticated`].
    #[doc(hidden)]
    #[must_use]
    pub fn new(verified: Arc<VerifiedJwt<C>>) -> Self {
        Self { verified }
    }

    /// Borrows the complete verified token retained by this principal.
    #[must_use]
    pub fn verified(&self) -> &VerifiedJwt<C> {
        &self.verified
    }
}

impl<C> Clone for ClaimsPrincipal<C> {
    fn clone(&self) -> Self {
        Self {
            verified: Arc::clone(&self.verified),
        }
    }
}

impl<C> Deref for ClaimsPrincipal<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.verified.claims.custom
    }
}

impl<C> fmt::Debug for ClaimsPrincipal<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClaimsPrincipal")
            .field("claims_type", &type_name::<C>())
            .finish()
    }
}

impl<C> PassportPrincipal for ClaimsPrincipal<C>
where
    C: PassportPrincipal,
{
    fn has_role(&self, role: &str) -> bool {
        self.verified.claims.custom.has_role(role)
    }

    fn has_permission(&self, permission: &str) -> bool {
        self.verified.claims.custom.has_permission(permission)
    }
}

/// An Axum extractor for the principal installed by a successful guard.
pub struct Authenticated<P>(Arc<P>);

impl<P> Authenticated<P> {
    #[allow(dead_code)]
    pub(crate) const fn new(principal: Arc<P>) -> Self {
        Self(principal)
    }
}

impl<P> Clone for Authenticated<P> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<P> Deref for Authenticated<P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<P> fmt::Debug for Authenticated<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Authenticated")
            .field("principal_type", &type_name::<P>())
            .finish()
    }
}

impl<P, S> FromRequestParts<S> for Authenticated<P>
where
    P: PassportPrincipal,
    S: Send + Sync,
{
    type Rejection = PassportRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Self>().cloned().ok_or_else(|| {
            PassportError::internal(MissingGuardExtension("authenticated principal")).into()
        })
    }
}

/// An Axum extractor for the complete token installed by a successful guard.
pub struct VerifiedToken<C>(Arc<VerifiedJwt<C>>);

impl<C> VerifiedToken<C> {
    #[allow(dead_code)]
    pub(crate) const fn new(verified: Arc<VerifiedJwt<C>>) -> Self {
        Self(verified)
    }

    /// Borrows the complete verified token.
    #[must_use]
    pub fn verified(&self) -> &VerifiedJwt<C> {
        &self.0
    }
}

impl<C> Clone for VerifiedToken<C> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<C> Deref for VerifiedToken<C> {
    type Target = VerifiedJwt<C>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C> fmt::Debug for VerifiedToken<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedToken")
            .field("claims_type", &type_name::<C>())
            .finish()
    }
}

impl<C, S> FromRequestParts<S> for VerifiedToken<C>
where
    C: Send + Sync + 'static,
    S: Send + Sync,
{
    type Rejection = PassportRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or_else(|| PassportError::internal(MissingGuardExtension("verified token")).into())
    }
}

#[derive(Debug)]
struct MissingGuardExtension(&'static str);

impl fmt::Display for MissingGuardExtension {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "missing guard-installed {}", self.0)
    }
}

impl std::error::Error for MissingGuardExtension {}

//! JWT algorithms, claims, headers, and operation options.

use std::any::type_name;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use super::{JwtError, JwtErrorKind};

/// A signing and verification algorithm supported by MADS.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum JwtAlgorithm {
    /// HMAC using SHA-256.
    Hs256,
    /// HMAC using SHA-384.
    Hs384,
    /// HMAC using SHA-512.
    Hs512,
    /// RSA PKCS#1 v1.5 using SHA-256.
    Rs256,
    /// RSA PKCS#1 v1.5 using SHA-384.
    Rs384,
    /// RSA PKCS#1 v1.5 using SHA-512.
    Rs512,
    /// ECDSA using the P-256 curve and SHA-256.
    Es256,
    /// ECDSA using the P-384 curve and SHA-384.
    Es384,
}

impl JwtAlgorithm {
    /// Returns the standard uppercase JWT algorithm name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hs256 => "HS256",
            Self::Hs384 => "HS384",
            Self::Hs512 => "HS512",
            Self::Rs256 => "RS256",
            Self::Rs384 => "RS384",
            Self::Rs512 => "RS512",
            Self::Es256 => "ES256",
            Self::Es384 => "ES384",
        }
    }

    #[allow(dead_code)]
    pub(super) const fn as_jsonwebtoken(self) -> jsonwebtoken::Algorithm {
        match self {
            Self::Hs256 => jsonwebtoken::Algorithm::HS256,
            Self::Hs384 => jsonwebtoken::Algorithm::HS384,
            Self::Hs512 => jsonwebtoken::Algorithm::HS512,
            Self::Rs256 => jsonwebtoken::Algorithm::RS256,
            Self::Rs384 => jsonwebtoken::Algorithm::RS384,
            Self::Rs512 => jsonwebtoken::Algorithm::RS512,
            Self::Es256 => jsonwebtoken::Algorithm::ES256,
            Self::Es384 => jsonwebtoken::Algorithm::ES384,
        }
    }

    #[allow(dead_code)]
    pub(super) const fn from_jsonwebtoken(algorithm: jsonwebtoken::Algorithm) -> Option<Self> {
        match algorithm {
            jsonwebtoken::Algorithm::HS256 => Some(Self::Hs256),
            jsonwebtoken::Algorithm::HS384 => Some(Self::Hs384),
            jsonwebtoken::Algorithm::HS512 => Some(Self::Hs512),
            jsonwebtoken::Algorithm::RS256 => Some(Self::Rs256),
            jsonwebtoken::Algorithm::RS384 => Some(Self::Rs384),
            jsonwebtoken::Algorithm::RS512 => Some(Self::Rs512),
            jsonwebtoken::Algorithm::ES256 => Some(Self::Es256),
            jsonwebtoken::Algorithm::ES384 => Some(Self::Es384),
            _ => None,
        }
    }
}

impl FromStr for JwtAlgorithm {
    type Err = JwtError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "HS256" => Ok(Self::Hs256),
            "HS384" => Ok(Self::Hs384),
            "HS512" => Ok(Self::Hs512),
            "RS256" => Ok(Self::Rs256),
            "RS384" => Ok(Self::Rs384),
            "RS512" => Ok(Self::Rs512),
            "ES256" => Ok(Self::Es256),
            "ES384" => Ok(Self::Es384),
            _ => Err(JwtError::new(JwtErrorKind::DisallowedAlgorithm)),
        }
    }
}

/// Whether a JWT is intended for access authorization or token refresh.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum JwtTokenKind {
    /// A short-lived access token.
    Access,
    /// A refresh token handled by application rotation policy.
    Refresh,
}

impl JwtTokenKind {
    /// Returns the private JWT `typ` profile used by MADS.
    pub const fn header_type(self) -> &'static str {
        match self {
            Self::Access => "mads-access+jwt",
            Self::Refresh => "mads-refresh+jwt",
        }
    }

    /// Returns the private `token_use` claim value used by MADS.
    pub const fn claim_value(self) -> &'static str {
        match self {
            Self::Access => "access",
            Self::Refresh => "refresh",
        }
    }
}

/// Registered JWT claims owned and validated by MADS.
#[derive(Clone, Eq, PartialEq)]
pub struct RegisteredJwtClaims {
    /// Token issuer (`iss`).
    pub issuer: Option<String>,
    /// Token subject (`sub`).
    pub subject: Option<String>,
    /// Token audiences (`aud`).
    pub audiences: Vec<String>,
    /// Expiration timestamp (`exp`) in Unix seconds.
    pub expires_at: u64,
    /// Optional not-before timestamp (`nbf`) in Unix seconds.
    pub not_before: Option<u64>,
    /// Issued-at timestamp (`iat`) in Unix seconds.
    pub issued_at: u64,
    /// Optional JWT identifier (`jti`).
    pub jwt_id: Option<String>,
    /// MADS access or refresh profile (`token_use`).
    pub token_kind: JwtTokenKind,
}

impl fmt::Debug for RegisteredJwtClaims {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegisteredJwtClaims")
            .field("has_issuer", &self.issuer.is_some())
            .field("has_subject", &self.subject.is_some())
            .field("audience_count", &self.audiences.len())
            .field("has_expiration", &true)
            .field("has_not_before", &self.not_before.is_some())
            .field("has_issued_at", &true)
            .field("has_jwt_id", &self.jwt_id.is_some())
            .field("token_kind", &self.token_kind)
            .finish()
    }
}

/// A JWT's MADS-owned registered claims and application-owned custom claims.
#[derive(Clone, Eq, PartialEq)]
pub struct JwtClaims<C> {
    /// Registered claims owned by MADS.
    pub registered: RegisteredJwtClaims,
    /// Typed custom claims owned by the application.
    pub custom: C,
}

impl<C> fmt::Debug for JwtClaims<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JwtClaims")
            .field("registered", &self.registered)
            .field("custom_type", &type_name::<C>())
            .finish()
    }
}

/// A validated JWT header and its typed claims.
#[derive(Clone, Eq, PartialEq)]
pub struct VerifiedJwt<C> {
    /// The validated protected header.
    pub header: JwtHeader,
    /// The validated registered and custom claims.
    pub claims: JwtClaims<C>,
}

impl<C> fmt::Debug for VerifiedJwt<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedJwt")
            .field("header", &self.header)
            .field("claims", &self.claims)
            .finish()
    }
}

/// The safe, supported portion of a JWT protected header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JwtHeader {
    /// The signing algorithm.
    pub algorithm: JwtAlgorithm,
    /// The optional named-key identifier (`kid`).
    pub key_id: Option<String>,
    /// The optional token type (`typ`).
    pub token_type: Option<String>,
}

/// Options required when MADS signs a JWT.
#[derive(Clone, Eq, PartialEq)]
pub struct JwtSignOptions {
    kind: JwtTokenKind,
    lifetime: Duration,
    not_before: Option<u64>,
    subject: Option<String>,
    jwt_id: Option<String>,
}

impl fmt::Debug for JwtSignOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JwtSignOptions")
            .field("kind", &self.kind)
            .field("lifetime", &self.lifetime)
            .field("has_not_before", &self.not_before.is_some())
            .field("has_subject", &self.subject.is_some())
            .field("has_jwt_id", &self.jwt_id.is_some())
            .finish()
    }
}

impl JwtSignOptions {
    /// Creates access-token signing options with the requested lifetime.
    pub const fn access(lifetime: Duration) -> Self {
        Self::new(JwtTokenKind::Access, lifetime)
    }

    /// Creates refresh-token signing options with the requested lifetime.
    pub const fn refresh(lifetime: Duration) -> Self {
        Self::new(JwtTokenKind::Refresh, lifetime)
    }

    const fn new(kind: JwtTokenKind, lifetime: Duration) -> Self {
        Self {
            kind,
            lifetime,
            not_before: None,
            subject: None,
            jwt_id: None,
        }
    }

    /// Sets the optional not-before timestamp in Unix seconds.
    pub const fn not_before(mut self, timestamp: u64) -> Self {
        self.not_before = Some(timestamp);
        self
    }

    /// Sets the optional subject.
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Sets the optional JWT identifier.
    pub fn jwt_id(mut self, jwt_id: impl Into<String>) -> Self {
        self.jwt_id = Some(jwt_id.into());
        self
    }

    /// Returns the token kind to issue.
    pub const fn kind(&self) -> JwtTokenKind {
        self.kind
    }

    /// Returns the requested lifetime.
    pub const fn lifetime(&self) -> Duration {
        self.lifetime
    }

    /// Returns the optional not-before timestamp.
    pub const fn not_before_value(&self) -> Option<u64> {
        self.not_before
    }

    /// Returns the optional subject.
    pub fn subject_value(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    /// Returns the optional JWT identifier.
    pub fn jwt_id_value(&self) -> Option<&str> {
        self.jwt_id.as_deref()
    }
}

/// Explicit verification requirements for one JWT operation.
#[derive(Clone, Eq, PartialEq)]
pub struct JwtValidation {
    kind: JwtTokenKind,
    require_subject: bool,
    subject: Option<String>,
    issuer: Option<String>,
    audience: Option<String>,
    require_jwt_id: bool,
    jwt_id: Option<String>,
}

impl fmt::Debug for JwtValidation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JwtValidation")
            .field("kind", &self.kind)
            .field("subject_required", &self.require_subject)
            .field("has_expected_subject", &self.subject.is_some())
            .field("has_expected_issuer", &self.issuer.is_some())
            .field("has_expected_audience", &self.audience.is_some())
            .field("jwt_id_required", &self.require_jwt_id)
            .field("has_expected_jwt_id", &self.jwt_id.is_some())
            .finish()
    }
}

impl JwtValidation {
    /// Creates validation requirements for an access token.
    pub const fn access() -> Self {
        Self::new(JwtTokenKind::Access)
    }

    /// Creates validation requirements for a refresh token.
    pub const fn refresh() -> Self {
        Self::new(JwtTokenKind::Refresh)
    }

    const fn new(kind: JwtTokenKind) -> Self {
        Self {
            kind,
            require_subject: false,
            subject: None,
            issuer: None,
            audience: None,
            require_jwt_id: false,
            jwt_id: None,
        }
    }

    /// Requires a subject claim without constraining its value.
    pub const fn require_subject(mut self) -> Self {
        self.require_subject = true;
        self
    }

    /// Requires the supplied subject value.
    pub fn subject(mut self, expected: impl Into<String>) -> Self {
        self.require_subject = true;
        self.subject = Some(expected.into());
        self
    }

    /// Requires the supplied issuer value.
    pub fn issuer(mut self, expected: impl Into<String>) -> Self {
        self.issuer = Some(expected.into());
        self
    }

    /// Requires the supplied audience value.
    pub fn audience(mut self, expected: impl Into<String>) -> Self {
        self.audience = Some(expected.into());
        self
    }

    /// Requires a JWT identifier without constraining its value.
    pub const fn require_jwt_id(mut self) -> Self {
        self.require_jwt_id = true;
        self
    }

    /// Requires the supplied JWT identifier value.
    pub fn jwt_id(mut self, expected: impl Into<String>) -> Self {
        self.require_jwt_id = true;
        self.jwt_id = Some(expected.into());
        self
    }

    /// Returns the required token kind.
    pub const fn kind(&self) -> JwtTokenKind {
        self.kind
    }

    /// Returns whether a subject claim is required.
    pub const fn subject_required(&self) -> bool {
        self.require_subject
    }

    /// Returns the required subject value, when constrained.
    pub fn subject_value(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    /// Returns the required issuer value, when constrained.
    pub fn issuer_value(&self) -> Option<&str> {
        self.issuer.as_deref()
    }

    /// Returns the required audience value, when constrained.
    pub fn audience_value(&self) -> Option<&str> {
        self.audience.as_deref()
    }

    /// Returns whether a JWT identifier claim is required.
    pub const fn jwt_id_required(&self) -> bool {
        self.require_jwt_id
    }

    /// Returns the required JWT identifier value, when constrained.
    pub fn jwt_id_value(&self) -> Option<&str> {
        self.jwt_id.as_deref()
    }
}

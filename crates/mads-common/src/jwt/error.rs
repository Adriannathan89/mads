//! Redacted JWT errors.

use std::fmt;

/// The result type used by JWT APIs.
pub type JwtResult<T> = std::result::Result<T, JwtError>;

/// A stable category for a JWT failure.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum JwtErrorKind {
    /// JWT configuration is missing, inconsistent, or invalid.
    InvalidConfiguration,
    /// Cryptographic key material is unreadable or malformed.
    InvalidKeyMaterial,
    /// No eligible signing key is available.
    UnavailableSigningKey,
    /// Claims could not be serialized.
    Serialization,
    /// Claims could not be deserialized.
    Deserialization,
    /// The token does not have a valid JWT representation.
    MalformedToken,
    /// The token exceeds the configured size limit.
    TokenTooLarge,
    /// The token uses an algorithm that is not allowed.
    DisallowedAlgorithm,
    /// The token algorithm does not match the selected key.
    AlgorithmMismatch,
    /// A required key identifier is absent.
    MissingKeyId,
    /// The key identifier does not name a configured verification key.
    UnknownKeyId,
    /// The token signature is invalid.
    InvalidSignature,
    /// The required expiration claim is absent.
    MissingExpiration,
    /// The token has expired.
    Expired,
    /// The required issued-at claim is absent.
    MissingIssuedAt,
    /// The token is not valid at the current time.
    InvalidNotBefore,
    /// The issuer does not match the validation policy.
    IssuerMismatch,
    /// The audience does not match the validation policy.
    AudienceMismatch,
    /// The subject does not match the validation policy.
    SubjectMismatch,
    /// The JWT identifier does not match the validation policy.
    JwtIdMismatch,
    /// The access or refresh token profile does not match the validation policy.
    TokenKindMismatch,
}

/// A normalized JWT error whose formatting never exposes sensitive context.
#[non_exhaustive]
pub struct JwtError {
    kind: JwtErrorKind,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl JwtError {
    /// Creates a JWT error with the supplied stable category.
    pub const fn new(kind: JwtErrorKind) -> Self {
        Self { kind, source: None }
    }

    /// Returns the stable category of this error.
    pub const fn kind(&self) -> JwtErrorKind {
        self.kind
    }

    #[allow(dead_code)]
    pub(crate) fn with_source<E>(kind: JwtErrorKind, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            kind,
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Debug for JwtError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JwtError")
            .field("kind", &self.kind)
            .field("has_source", &self.source.is_some())
            .finish()
    }
}

impl fmt::Display for JwtError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            JwtErrorKind::InvalidConfiguration => "JWT configuration is invalid",
            JwtErrorKind::InvalidKeyMaterial => "JWT key material is invalid",
            JwtErrorKind::UnavailableSigningKey => "JWT signing key is unavailable",
            JwtErrorKind::Serialization => "JWT claims could not be serialized",
            JwtErrorKind::Deserialization => "JWT claims could not be deserialized",
            JwtErrorKind::MalformedToken => "JWT is malformed",
            JwtErrorKind::TokenTooLarge => "JWT exceeds the allowed size",
            JwtErrorKind::DisallowedAlgorithm => "JWT algorithm is not allowed",
            JwtErrorKind::AlgorithmMismatch => "JWT algorithm does not match the key",
            JwtErrorKind::MissingKeyId => "JWT key identifier is required",
            JwtErrorKind::UnknownKeyId => "JWT key identifier is not recognized",
            JwtErrorKind::InvalidSignature => "JWT signature is invalid",
            JwtErrorKind::MissingExpiration => "JWT expiration is required",
            JwtErrorKind::Expired => "JWT has expired",
            JwtErrorKind::MissingIssuedAt => "JWT issued-at time is required",
            JwtErrorKind::InvalidNotBefore => "JWT is not currently valid",
            JwtErrorKind::IssuerMismatch => "JWT issuer does not match",
            JwtErrorKind::AudienceMismatch => "JWT audience does not match",
            JwtErrorKind::SubjectMismatch => "JWT subject does not match",
            JwtErrorKind::JwtIdMismatch => "JWT identifier does not match",
            JwtErrorKind::TokenKindMismatch => "JWT token kind does not match",
        })
    }
}

impl std::error::Error for JwtError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

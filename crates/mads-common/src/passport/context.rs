//! Read-only request context supplied to Passport strategies.

use std::{fmt, net::SocketAddr};

use axum::http::{
    HeaderMap, Method, Uri,
    header::{AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION},
};

#[cfg(feature = "cookies")]
use crate::{Cookie, CookieJar};

/// A read-only parsed-cookie view that excludes the cookie used for authentication.
///
/// Passport supplies this view only for cookie-authenticated requests. It allows
/// strategies to inspect other parsed request cookies without disclosing the raw
/// token selected by the guard.
#[cfg(feature = "cookies")]
pub struct PassportCookies<'a> {
    cookies: &'a CookieJar,
    excluded_name: Option<String>,
}

#[cfg(feature = "cookies")]
impl<'a> PassportCookies<'a> {
    const fn new(cookies: &'a CookieJar, excluded_name: Option<String>) -> Self {
        Self {
            cookies,
            excluded_name,
        }
    }

    /// Returns a permitted request cookie by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Cookie<'static>> {
        (!self.is_excluded(name))
            .then(|| self.cookies.get(name))
            .flatten()
    }

    /// Iterates over permitted request cookies.
    pub fn iter(&self) -> impl Iterator<Item = &'_ Cookie<'static>> {
        self.cookies
            .iter()
            .filter(|cookie| !self.is_excluded(cookie.name()))
    }

    /// Returns how many times a permitted cookie occurred in the request.
    #[must_use]
    pub fn occurrences(&self, name: &str) -> usize {
        if self.is_excluded(name) {
            0
        } else {
            self.cookies.occurrences(name)
        }
    }

    fn is_excluded(&self, name: &str) -> bool {
        self.excluded_name.as_deref() == Some(name)
    }
}

#[cfg(feature = "cookies")]
impl fmt::Debug for PassportCookies<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PassportCookies")
            .field("has_excluded_cookie", &self.excluded_name.is_some())
            .finish_non_exhaustive()
    }
}

/// A read-only view of request metadata available during strategy validation.
pub struct PassportContext<'a> {
    headers: HeaderMap,
    method: &'a Method,
    uri: &'a Uri,
    remote_addr: Option<SocketAddr>,
    #[cfg(feature = "cookies")]
    cookies: Option<PassportCookies<'a>>,
}

impl<'a> PassportContext<'a> {
    /// Creates a safe metadata view for Bearer authentication.
    ///
    /// The context strips credential-bearing headers before application strategy
    /// code receives it. Cookie-authenticated contexts are constructed
    /// separately so their selected token remains hidden from the parsed-cookie
    /// view as well.
    #[allow(dead_code)]
    pub(crate) fn new(
        headers: &'a HeaderMap,
        method: &'a Method,
        uri: &'a Uri,
        remote_addr: Option<SocketAddr>,
    ) -> Self {
        Self::from_parts(
            headers,
            method,
            uri,
            remote_addr,
            #[cfg(feature = "cookies")]
            None,
            #[cfg(feature = "cookies")]
            None,
        )
    }

    /// Creates a safe metadata view for cookie authentication.
    ///
    /// The selected cookie name is excluded from the strategy-facing cookie
    /// view, and credential-bearing headers are removed in the same way as
    /// [`Self::new`].
    #[cfg(feature = "cookies")]
    #[allow(dead_code)]
    pub(crate) fn with_cookie_token(
        headers: &'a HeaderMap,
        method: &'a Method,
        uri: &'a Uri,
        remote_addr: Option<SocketAddr>,
        cookies: &'a CookieJar,
        token_cookie: &str,
    ) -> Self {
        Self::from_parts(
            headers,
            method,
            uri,
            remote_addr,
            Some(cookies),
            Some(token_cookie.to_owned()),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        headers: &'a HeaderMap,
        method: &'a Method,
        uri: &'a Uri,
        remote_addr: Option<SocketAddr>,
        #[cfg(feature = "cookies")] cookies: Option<&'a CookieJar>,
        #[cfg(feature = "cookies")] excluded_cookie: Option<String>,
    ) -> Self {
        Self {
            headers: safe_headers(headers),
            method,
            uri,
            remote_addr,
            #[cfg(feature = "cookies")]
            cookies: cookies.map(|cookies| PassportCookies::new(cookies, excluded_cookie)),
        }
    }

    /// Returns safe request headers without authentication credentials.
    ///
    /// The returned copy excludes `Authorization`, `Proxy-Authorization`, and
    /// `Cookie` so a strategy cannot read a raw framework authentication token.
    #[must_use]
    pub const fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns the request method.
    #[must_use]
    pub const fn method(&self) -> &'a Method {
        self.method
    }

    /// Returns the request URI.
    #[must_use]
    pub const fn uri(&self) -> &'a Uri {
        self.uri
    }

    /// Returns the peer address when the HTTP runtime supplied one.
    #[must_use]
    pub const fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }

    /// Returns permitted parsed request cookies when cookie support is enabled.
    ///
    /// For cookie authentication, the selected token cookie is absent from this
    /// view.
    #[cfg(feature = "cookies")]
    #[must_use]
    pub const fn cookies(&self) -> Option<&PassportCookies<'a>> {
        self.cookies.as_ref()
    }
}

fn safe_headers(headers: &HeaderMap) -> HeaderMap {
    let mut safe = headers.clone();
    safe.remove(AUTHORIZATION);
    safe.remove(PROXY_AUTHORIZATION);
    safe.remove(COOKIE);
    safe
}

impl fmt::Debug for PassportContext<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("PassportContext");
        debug
            .field("header_count", &self.headers.len())
            .field("method", &self.method)
            .field("has_remote_addr", &self.remote_addr.is_some());
        #[cfg(feature = "cookies")]
        debug.field("has_cookies", &self.cookies.is_some());
        debug.finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{
        HeaderMap, HeaderValue, Method, Uri,
        header::{AUTHORIZATION, COOKIE},
    };

    use super::PassportContext;
    use crate::{
        JwtClaims, JwtTokenKind, PassportPrincipal, PassportResult, PassportStrategy,
        RegisteredJwtClaims,
    };

    struct ContextProbePrincipal;

    impl PassportPrincipal for ContextProbePrincipal {
        fn has_role(&self, _role: &str) -> bool {
            false
        }

        fn has_permission(&self, _permission: &str) -> bool {
            false
        }
    }

    struct BearerContextProbe;

    impl PassportStrategy for BearerContextProbe {
        type Claims = ();
        type Principal = ContextProbePrincipal;

        const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

        async fn validate(
            &self,
            context: &PassportContext<'_>,
            _claims: &JwtClaims<Self::Claims>,
        ) -> PassportResult<Self::Principal> {
            assert_eq!(context.method(), Method::GET);
            assert_eq!(context.uri(), &Uri::from_static("/profile"));
            assert_eq!(context.remote_addr().unwrap().to_string(), "127.0.0.1:8443");
            assert_eq!(
                context.headers().get("x-request-id").unwrap(),
                "request-123"
            );
            assert!(context.headers().get(AUTHORIZATION).is_none());
            assert!(context.headers().get(COOKIE).is_none());
            assert!(context.headers().get("proxy-authorization").is_none());
            Ok(ContextProbePrincipal)
        }
    }

    fn claims() -> JwtClaims<()> {
        JwtClaims {
            registered: RegisteredJwtClaims {
                issuer: None,
                subject: None,
                audiences: Vec::new(),
                expires_at: 2,
                not_before: None,
                issued_at: 1,
                jwt_id: None,
                token_kind: JwtTokenKind::Access,
            },
            custom: (),
        }
    }

    #[tokio::test]
    async fn strategy_context_hides_the_raw_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer bearer-token-sentinel"),
        );
        headers.insert(
            COOKIE,
            HeaderValue::from_static("access_token=cookie-token-sentinel"),
        );
        headers.insert(
            "proxy-authorization",
            HeaderValue::from_static("Bearer proxy-token-sentinel"),
        );
        headers.insert("x-request-id", HeaderValue::from_static("request-123"));
        let method = Method::GET;
        let uri = Uri::from_static("/profile");
        let context = PassportContext::new(
            &headers,
            &method,
            &uri,
            Some("127.0.0.1:8443".parse().unwrap()),
        );

        BearerContextProbe
            .validate(&context, &claims())
            .await
            .expect("the strategy should receive safe request metadata");
    }

    #[cfg(feature = "cookies")]
    struct CookieContextProbe;

    #[cfg(feature = "cookies")]
    impl PassportStrategy for CookieContextProbe {
        type Claims = ();
        type Principal = ContextProbePrincipal;

        const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

        async fn validate(
            &self,
            context: &PassportContext<'_>,
            _claims: &JwtClaims<Self::Claims>,
        ) -> PassportResult<Self::Principal> {
            let cookies = context
                .cookies()
                .expect("cookie-authenticated strategies receive permitted cookies");
            assert!(context.headers().get(COOKIE).is_none());
            assert_eq!(
                cookies.get("theme").map(|cookie| cookie.value()),
                Some("dark")
            );
            assert_eq!(cookies.occurrences("theme"), 1);
            assert!(cookies.get("access_token").is_none());
            assert_eq!(cookies.occurrences("access_token"), 0);
            assert_eq!(
                cookies
                    .iter()
                    .map(|cookie| cookie.name())
                    .collect::<Vec<_>>(),
                ["theme"],
            );
            Ok(ContextProbePrincipal)
        }
    }

    #[cfg(feature = "cookies")]
    #[tokio::test]
    async fn strategy_context_hides_the_selected_cookie_token() {
        use crate::CookieJar;

        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_static("access_token=cookie-token-sentinel; theme=dark"),
        );
        let cookies = CookieJar::from_headers(&headers).expect("the request cookies are valid");
        let method = Method::GET;
        let uri = Uri::from_static("/profile");
        let context = PassportContext::with_cookie_token(
            &headers,
            &method,
            &uri,
            None,
            &cookies,
            "access_token",
        );

        CookieContextProbe
            .validate(&context, &claims())
            .await
            .expect("the strategy should receive permitted cookies only");
    }
}

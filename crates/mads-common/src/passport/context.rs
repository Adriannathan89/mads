//! Read-only request context supplied to Passport strategies.

use std::{fmt, net::SocketAddr};

use axum::http::{HeaderMap, Method, Uri};

#[cfg(feature = "cookies")]
use crate::CookieJar;

/// A read-only view of request metadata available during strategy validation.
pub struct PassportContext<'a> {
    headers: &'a HeaderMap,
    method: &'a Method,
    uri: &'a Uri,
    remote_addr: Option<SocketAddr>,
    #[cfg(feature = "cookies")]
    cookies: Option<&'a CookieJar>,
}

impl<'a> PassportContext<'a> {
    #[allow(dead_code)]
    pub(crate) const fn new(
        headers: &'a HeaderMap,
        method: &'a Method,
        uri: &'a Uri,
        remote_addr: Option<SocketAddr>,
        #[cfg(feature = "cookies")] cookies: Option<&'a CookieJar>,
    ) -> Self {
        Self {
            headers,
            method,
            uri,
            remote_addr,
            #[cfg(feature = "cookies")]
            cookies,
        }
    }

    /// Returns the request headers without exposing them through debug output.
    #[must_use]
    pub const fn headers(&self) -> &'a HeaderMap {
        self.headers
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

    /// Returns the parsed request cookies when cookie support is enabled.
    #[cfg(feature = "cookies")]
    #[must_use]
    pub const fn cookies(&self) -> Option<&'a CookieJar> {
        self.cookies
    }
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

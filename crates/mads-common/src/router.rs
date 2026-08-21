//! Validated Axum router construction for managed controllers.
//!
//! Controllers are resolved from the application context once while the router
//! is built. Generated handlers capture that application-scoped controller
//! handle and use typed trait calls for each request.

use crate::RouteCatalog;

/// Builds an Axum router from every validated managed-controller registration.
///
/// The complete route catalog is validated before any generated registrar is
/// invoked, so invalid metadata cannot install a partial router.
#[allow(clippy::result_large_err)]
pub fn build_router(application: &mads_core::Mads) -> mads_core::Result<axum::Router> {
    let controllers = RouteCatalog::validated()?;
    let mut router = axum::Router::new();
    for controller in controllers {
        let mut routes = controller.routes();
        router = (controller.registrar())(router, application.context(), &mut routes)?;
        routes.finish()?;
    }
    Ok(router)
}

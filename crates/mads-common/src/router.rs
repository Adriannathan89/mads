//! Validated Axum router construction for managed controllers.
//!
//! Controllers are resolved from the application context once while the router
//! is built. Generated handlers capture that application-scoped controller
//! handle and use typed trait calls for each request.

use crate::{
    cors::CorsPlan,
    http_scope::HttpApplicationScope,
    route::{RouterBuildContext, validate_scoped_descriptors},
};

#[cfg(feature = "jwt")]
use crate::PassportStrategyCatalog;

/// Builds an Axum router from the application's validated managed-controller registrations.
///
/// The selected route scope is validated before any generated registrar is
/// invoked, so invalid metadata cannot install a partial router. Each
/// application-scoped controller is resolved once while the router is being
/// assembled; requests then use the generated typed trait calls.
///
/// # Errors
///
/// Returns [`mads_core::Error`] when route metadata is invalid, a generated
/// registrar reports a framework error, or a controller cannot be resolved
/// from the application's construction context. Validation errors use the
/// `MADS030` diagnostic code and occur before any Axum route is installed.
///
/// # Examples
///
/// ```no_run
/// # use mads_common::core::Mads;
/// # use mads_common::build_router;
/// #
/// # #[tokio::main]
/// # async fn main() -> mads_common::core::Result<()> {
/// let application = Mads::builder().build().await?;
/// let router = build_router(&application)?;
/// let _ = router;
/// # Ok(())
/// # }
/// ```
#[allow(clippy::result_large_err)]
pub fn build_router(application: &mads_core::Mads) -> mads_core::Result<axum::Router> {
    let http_scope = HttpApplicationScope::for_application(application)?;
    #[cfg(feature = "jwt")]
    let passport =
        PassportStrategyCatalog::preflight_scoped(application.module_graph(), http_scope.guards())?;
    let runtime = RouterBuildContext::new(
        application.context(),
        #[cfg(feature = "jwt")]
        &passport,
    );
    let controllers = validate_scoped_descriptors(http_scope.controllers())?;
    let mut router = axum::Router::new();
    for controller in controllers {
        let mut routes = controller.routes();
        router = (controller.registrar())(router, &runtime, &mut routes)?;
        routes.finish()?;
    }
    Ok(router)
}

/// Applies final application-wide router configuration to a complete raw router.
///
/// Merge generated and native routes before calling this function so that
/// application-wide middleware, including configured CORS, wraps every route.
/// When CORS is absent, the raw router is returned unchanged. Callers that use
/// a router directly configure it here; callers that pass a router to
/// `serve_router` pass the raw router instead. Applying both paths would
/// register the application-wide layer twice.
///
/// # Errors
///
/// Returns [`mads_core::Error`] when the application context cannot resolve
/// its configured CORS plan for a reason other than CORS being absent.
#[allow(clippy::result_large_err)]
pub fn configure_router(
    application: &mads_core::Mads,
    router: axum::Router,
) -> mads_core::Result<axum::Router> {
    match application.context().resolve::<CorsPlan>() {
        Ok(cors) => Ok(router.layer(cors.layer())),
        Err(error) if error.code() == mads_core::MADS003 => Ok(router),
        Err(error) => Err(error),
    }
}

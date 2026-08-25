//! Validated HTTP server startup and lifecycle coordination.
//!
//! [`serve`] validates route metadata and builds the complete router before it
//! starts application lifecycle hooks or asks Tokio to bind a listener.

use std::error::Error as StdError;
use std::fmt;
use std::future::Future;

use mads_core::Mads;
use tokio::net::TcpListener;

use crate::build_router;

/// An error produced while preparing, running, or stopping the HTTP runtime.
#[derive(Debug)]
#[non_exhaustive]
pub enum HttpRuntimeError {
    /// Route validation or router construction failed before lifecycle startup.
    Bootstrap(mads_core::Error),
    /// Application lifecycle startup or shutdown failed.
    Lifecycle(mads_core::Error),
    /// The HTTP listener could not bind to the requested address.
    Bind(std::io::Error),
    /// Axum stopped because of an HTTP serving failure.
    Serve(std::io::Error),
    /// An operational failure occurred and the subsequent shutdown also failed.
    OperationAndShutdown {
        /// The bind or serving error that initiated shutdown.
        operation: Box<HttpRuntimeError>,
        /// The lifecycle error returned while attempting shutdown.
        shutdown: mads_core::Error,
    },
}

impl fmt::Display for HttpRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bootstrap(error) => write!(formatter, "HTTP bootstrap failed: {error}"),
            Self::Lifecycle(error) => write!(formatter, "application lifecycle failed: {error}"),
            Self::Bind(error) => write!(formatter, "HTTP listener bind failed: {error}"),
            Self::Serve(error) => write!(formatter, "HTTP serving failed: {error}"),
            Self::OperationAndShutdown {
                operation,
                shutdown,
            } => write!(
                formatter,
                "{operation}; application shutdown also failed: {shutdown}"
            ),
        }
    }
}

impl StdError for HttpRuntimeError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Bootstrap(error) | Self::Lifecycle(error) => Some(error),
            Self::Bind(error) | Self::Serve(error) => Some(error),
            Self::OperationAndShutdown { operation, .. } => Some(operation.as_ref()),
        }
    }
}

/// Validates, starts, serves, and shuts down an application on `address`.
///
/// Route validation completes before lifecycle hooks start or the listener is
/// bound. Once lifecycle startup succeeds, every exit path attempts shutdown.
/// A bind or serving failure is retained if shutdown succeeds; if shutdown
/// also fails, both failures are returned in [`HttpRuntimeError::OperationAndShutdown`].
///
/// # Errors
///
/// Returns [`HttpRuntimeError::Bootstrap`] for route validation, controller
/// resolution, or registrar failures; [`HttpRuntimeError::Lifecycle`] for
/// lifecycle start or clean-shutdown failures; [`HttpRuntimeError::Bind`] when
/// the address cannot be bound; [`HttpRuntimeError::Serve`] for an Axum serving
/// failure; or [`HttpRuntimeError::OperationAndShutdown`] when an operational
/// failure and its cleanup failure occur together.
///
/// # Examples
///
/// ```no_run
/// use mads_common::{core::Mads, serve};
///
/// #[tokio::main]
/// async fn main() -> Result<(), mads_common::HttpRuntimeError> {
///     let application = Mads::builder().build().await.map_err(
///         mads_common::HttpRuntimeError::Bootstrap,
///     )?;
///     serve(application, "127.0.0.1:3000").await
/// }
/// ```
#[allow(clippy::result_large_err)]
pub async fn serve(
    application: Mads,
    address: impl tokio::net::ToSocketAddrs,
) -> Result<(), HttpRuntimeError> {
    serve_with(application, address, TcpListener::bind, shutdown_signal()).await
}

async fn serve_with<Address, B, BindFuture, Shutdown>(
    mut application: Mads,
    address: Address,
    binder: B,
    shutdown: Shutdown,
) -> Result<(), HttpRuntimeError>
where
    B: FnOnce(Address) -> BindFuture,
    BindFuture: Future<Output = std::io::Result<TcpListener>>,
    Shutdown: Future<Output = ()> + Send + 'static,
{
    let router = build_router(&application).map_err(HttpRuntimeError::Bootstrap)?;
    application
        .start()
        .await
        .map_err(HttpRuntimeError::Lifecycle)?;
    let listener = match binder(address).await {
        Ok(listener) => listener,
        Err(error) => {
            return finish_after_error(application, HttpRuntimeError::Bind(error)).await;
        }
    };
    let result = axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(HttpRuntimeError::Serve);
    finish(application, result).await
}

async fn finish(
    mut application: Mads,
    operation: Result<(), HttpRuntimeError>,
) -> Result<(), HttpRuntimeError> {
    match (operation, application.shutdown().await) {
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(shutdown)) => Err(HttpRuntimeError::Lifecycle(shutdown)),
        (Err(operation), Ok(())) => Err(operation),
        (Err(operation), Err(shutdown)) => Err(HttpRuntimeError::OperationAndShutdown {
            operation: Box::new(operation),
            shutdown,
        }),
    }
}

async fn finish_after_error(
    application: Mads,
    operation: HttpRuntimeError,
) -> Result<(), HttpRuntimeError> {
    finish(application, Err(operation)).await
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    #[cfg(feature = "database")]
    use std::error::Error as StdError;
    use std::io;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use mads_core::{
        ApplicationContext, Diagnostic, Error, LifecycleFuture, LifecycleHook, MADS011, MADS020,
        Mads, SourceLocation,
    };
    #[cfg(feature = "database")]
    use mads_core::{AutoConfigurationStatus, ConfigBuilder, MapSource};
    #[cfg(feature = "database")]
    use tokio::net::TcpListener;

    use super::{HttpRuntimeError, serve_with};
    use crate::{ControllerRouteDescriptor, HttpMethod, RouteContractDescriptor, RouteDescriptor};
    #[cfg(feature = "database")]
    use crate::{Database, DatabaseConfig, DatabaseErrorKind, MADS100, MadsBuilderDatabaseExt};

    #[cfg(feature = "database")]
    const FAILING_MIGRATIONS: diesel_migrations::EmbeddedMigrations =
        diesel_migrations::embed_migrations!("tests/fixtures/failing_migrations");

    static STARTS: AtomicUsize = AtomicUsize::new(0);
    static BINDS: AtomicUsize = AtomicUsize::new(0);
    static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    struct PreflightController;
    struct PreflightPermit;

    #[cfg(feature = "database")]
    #[mads_core::repository]
    struct AutoDatabaseRepository {
        database: Database,
    }

    #[cfg(feature = "database")]
    impl AutoDatabaseRepository {
        fn database(&self) -> &Database {
            &self.database
        }
    }

    fn preflight_controller_type_id() -> TypeId {
        TypeId::of::<PreflightController>()
    }

    fn preflight_registrar(
        router: axum::Router,
        context: &ApplicationContext,
        routes: &mut crate::__private::ValidatedRouteIter<'_>,
    ) -> mads_core::Result<axum::Router> {
        let _ = context.resolve::<PreflightPermit>()?;
        let path = routes.next(HttpMethod::Get, "health")?;
        routes.finish()?;
        Ok(router.route(path, axum::routing::get(|| async { "ok" })))
    }

    mads_core::__private::inventory::submit! {
        ControllerRouteDescriptor::with_registrar(
            "server_tests::PreflightController",
            preflight_controller_type_id,
            &[RouteContractDescriptor::new(
                "HealthRoutes",
                &[RouteDescriptor::new(
                    HttpMethod::Get,
                    "",
                    "/health",
                    "/health",
                    "health",
                    SourceLocation::new(file!(), line!(), column!()),
                )],
            )],
            preflight_registrar,
        )
    }

    struct RecordingHook {
        events: Arc<Mutex<Vec<&'static str>>>,
        fail_shutdown: bool,
    }

    impl LifecycleHook for RecordingHook {
        fn name(&self) -> &str {
            "server-test"
        }

        fn start<'a>(&'a self, _: &'a ApplicationContext) -> LifecycleFuture<'a> {
            Box::pin(async move {
                STARTS.fetch_add(1, Ordering::SeqCst);
                self.events.lock().unwrap().push("start");
                Ok(())
            })
        }

        fn stop<'a>(&'a self, _: &'a ApplicationContext) -> LifecycleFuture<'a> {
            Box::pin(async move {
                self.events.lock().unwrap().push("shutdown");
                if self.fail_shutdown {
                    Err(test_core_error("shutdown failed"))
                } else {
                    Ok(())
                }
            })
        }
    }

    fn test_core_error(message: &str) -> Error {
        Error::new(Diagnostic::new(MADS020, "server test failure", message))
    }

    async fn application(
        events: Arc<Mutex<Vec<&'static str>>>,
        preflight_permitted: bool,
        fail_shutdown: bool,
    ) -> Mads {
        let mut builder = Mads::builder();
        #[cfg(feature = "database")]
        builder
            .provide(
                Database::from_config(
                    &DatabaseConfig::new("postgres://127.0.0.1:1/server-test").unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        builder.lifecycle_hook(RecordingHook {
            events,
            fail_shutdown,
        });
        if preflight_permitted {
            builder.provide(PreflightPermit).unwrap();
        }
        builder.build().await.unwrap()
    }

    fn address() -> SocketAddr {
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
    }

    #[tokio::test]
    async fn preflight_failure_prevents_lifecycle_start_and_bind() {
        let _guard = TEST_LOCK.lock().await;
        STARTS.store(0, Ordering::SeqCst);
        BINDS.store(0, Ordering::SeqCst);
        let events = Arc::new(Mutex::new(Vec::new()));
        let application = application(Arc::clone(&events), false, false).await;
        let binder = |address| async move {
            BINDS.fetch_add(1, Ordering::SeqCst);
            tokio::net::TcpListener::bind(address).await
        };

        let error = serve_with(application, address(), binder, async {})
            .await
            .unwrap_err();

        assert!(matches!(error, HttpRuntimeError::Bootstrap(_)));
        assert_eq!(STARTS.load(Ordering::SeqCst), 0);
        assert_eq!(BINDS.load(Ordering::SeqCst), 0);
        assert!(events.lock().unwrap().is_empty());
    }

    #[cfg(feature = "database")]
    #[tokio::test]
    async fn database_start_failure_prevents_listener_binding() {
        let _guard = TEST_LOCK.lock().await;
        let database_url = "postgres://127.0.0.1:1/mads";
        let config = ConfigBuilder::new()
            .source(MapSource::new("test", [("database.url", database_url)]))
            .build()
            .unwrap();
        let mut builder = Mads::builder_with_config(config);
        builder.provide(PreflightPermit).unwrap();
        let application = builder.build().await.unwrap();
        assert_eq!(
            application.auto_configurations()[0].status(),
            AutoConfigurationStatus::Active,
        );
        BINDS.store(0, Ordering::SeqCst);

        let error = serve_with(
            application,
            address(),
            |_| async {
                BINDS.fetch_add(1, Ordering::SeqCst);
                tokio::net::TcpListener::bind(address()).await
            },
            async {},
        )
        .await
        .unwrap_err();

        match &error {
            HttpRuntimeError::Lifecycle(error) => {
                assert_eq!(error.code(), MADS011);
                let source = std::error::Error::source(error)
                    .unwrap()
                    .downcast_ref::<Error>()
                    .unwrap();
                assert_eq!(source.code(), MADS100);
            }
            other => panic!("expected lifecycle failure, got {other:?}"),
        }
        assert_eq!(BINDS.load(Ordering::SeqCst), 0);
        let output = format!("{error}\n{error:?}");
        assert!(!output.contains(database_url));
    }

    #[cfg(feature = "database")]
    #[tokio::test]
    #[ignore = "requires PostgreSQL through MADS_TEST_DATABASE_URL"]
    async fn database_migration_failure_prevents_listener_binding() {
        let _guard = TEST_LOCK.lock().await;
        STARTS.store(0, Ordering::SeqCst);
        BINDS.store(0, Ordering::SeqCst);
        let events = Arc::new(Mutex::new(Vec::new()));
        let database_url = std::env::var("MADS_TEST_DATABASE_URL")
            .expect("MADS_TEST_DATABASE_URL is required for ignored PostgreSQL tests");
        let config = ConfigBuilder::new()
            .source(MapSource::new(
                "test",
                [
                    ("database.url", database_url.clone()),
                    ("database.migrate", "true".to_owned()),
                ],
            ))
            .build()
            .unwrap();
        let mut builder = Mads::builder_with_config(config);
        builder.provide(PreflightPermit).unwrap();
        builder.lifecycle_hook(RecordingHook {
            events: Arc::clone(&events),
            fail_shutdown: false,
        });
        builder.database_migrations(FAILING_MIGRATIONS).unwrap();
        let application = builder.build().await.unwrap();
        let database = application.context().resolve::<Database>().unwrap();

        let error = serve_with(
            application,
            address(),
            |address| async move {
                BINDS.fetch_add(1, Ordering::SeqCst);
                TcpListener::bind(address).await
            },
            async {},
        )
        .await
        .unwrap_err();

        match &error {
            HttpRuntimeError::Lifecycle(error) => {
                assert_eq!(error.code(), MADS011);
                let bootstrap_error = StdError::source(error)
                    .expect("MADS011 lifecycle errors retain their database bootstrap source")
                    .downcast_ref::<Error>()
                    .expect("MADS011 source is the database bootstrap error");
                assert_eq!(bootstrap_error.code(), MADS100);
                let database_source = StdError::source(bootstrap_error)
                    .expect("MADS100 errors retain their database error source");
                assert_eq!(
                    database_source.to_string(),
                    "database migration failed",
                    "the source chain must retain DatabaseErrorKind::{:?}",
                    DatabaseErrorKind::Migration,
                );
                assert!(
                    format!("{database_source:?}")
                        .contains(&format!("{:?}", DatabaseErrorKind::Migration))
                );
            }
            other => panic!("expected lifecycle failure, got {other:?}"),
        }
        assert_eq!(STARTS.load(Ordering::SeqCst), 0);
        assert_eq!(BINDS.load(Ordering::SeqCst), 0);
        assert!(events.lock().unwrap().is_empty());
        assert!(database.is_closed());
        let output = format!("{error}\n{error:?}");
        assert!(!output.contains(&database_url));
    }

    #[cfg(feature = "database")]
    #[tokio::test]
    async fn invalid_routes_prevent_automatic_database_checkout_and_binding() {
        let _guard = TEST_LOCK.lock().await;
        STARTS.store(0, Ordering::SeqCst);
        BINDS.store(0, Ordering::SeqCst);
        let events = Arc::new(Mutex::new(Vec::new()));
        let config = ConfigBuilder::new()
            .source(MapSource::new(
                "test",
                [("database.url", "postgres://127.0.0.1:1/mads")],
            ))
            .build()
            .unwrap();
        let mut builder = Mads::builder_with_config(config);
        builder.lifecycle_hook(RecordingHook {
            events: Arc::clone(&events),
            fail_shutdown: false,
        });
        let application = builder.build().await.unwrap();
        assert_eq!(
            application.auto_configurations()[0].status(),
            AutoConfigurationStatus::Active,
        );
        let database = application.context().resolve::<Database>().unwrap();
        let repository = application
            .context()
            .resolve::<AutoDatabaseRepository>()
            .unwrap();
        assert_eq!(repository.database().status().size(), 0);

        let error = serve_with(
            application,
            address(),
            |address| async move {
                BINDS.fetch_add(1, Ordering::SeqCst);
                TcpListener::bind(address).await
            },
            async {},
        )
        .await
        .unwrap_err();

        assert!(matches!(error, HttpRuntimeError::Bootstrap(_)));
        assert_eq!(STARTS.load(Ordering::SeqCst), 0);
        assert_eq!(BINDS.load(Ordering::SeqCst), 0);
        assert_eq!(database.status().size(), 0);
        assert!(events.lock().unwrap().is_empty());
        database.close();
    }

    #[tokio::test]
    async fn lifecycle_starts_before_bind_and_shuts_down_after_serving() {
        let _guard = TEST_LOCK.lock().await;
        STARTS.store(0, Ordering::SeqCst);
        let events = Arc::new(Mutex::new(Vec::new()));
        let application = application(Arc::clone(&events), true, false).await;
        let binder_events = Arc::clone(&events);
        let binder = move |address| async move {
            binder_events.lock().unwrap().push("bind");
            tokio::net::TcpListener::bind(address).await
        };

        serve_with(application, address(), binder, async {})
            .await
            .unwrap();

        assert_eq!(*events.lock().unwrap(), ["start", "bind", "shutdown"]);
    }

    #[tokio::test]
    async fn bind_failure_still_attempts_shutdown() {
        let _guard = TEST_LOCK.lock().await;
        let events = Arc::new(Mutex::new(Vec::new()));
        let application = application(Arc::clone(&events), true, false).await;
        let binder_events = Arc::clone(&events);
        let binder = move |_| async move {
            binder_events.lock().unwrap().push("bind");
            Err(io::Error::new(io::ErrorKind::AddrInUse, "occupied"))
        };

        let error = serve_with(application, address(), binder, async {})
            .await
            .unwrap_err();

        assert!(matches!(error, HttpRuntimeError::Bind(_)));
        assert_eq!(*events.lock().unwrap(), ["start", "bind", "shutdown"]);
    }

    #[tokio::test]
    async fn operational_and_shutdown_failures_are_both_retained() {
        let _guard = TEST_LOCK.lock().await;
        let events = Arc::new(Mutex::new(Vec::new()));
        let application = application(Arc::clone(&events), true, true).await;
        let binder = |_| async {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "bind denied",
            ))
        };

        let error = serve_with(application, address(), binder, async {})
            .await
            .unwrap_err();

        match error {
            HttpRuntimeError::OperationAndShutdown {
                operation,
                shutdown,
            } => {
                assert!(matches!(*operation, HttpRuntimeError::Bind(_)));
                assert_eq!(shutdown.code(), MADS011);
                let source = std::error::Error::source(&shutdown)
                    .unwrap()
                    .downcast_ref::<Error>()
                    .unwrap();
                assert_eq!(source.code(), MADS020);
            }
            other => panic!("expected combined failure, got {other:?}"),
        }
        assert_eq!(*events.lock().unwrap(), ["start", "shutdown"]);
    }
}

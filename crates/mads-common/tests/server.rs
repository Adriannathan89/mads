//! Public HTTP runtime error contract tests.

#![cfg(feature = "http")]

use std::error::Error as _;
use std::io;

use mads_common::core::{Diagnostic, Error, MADS020, Mads};
use mads_common::{HttpRuntimeError, MadsRunExt, serve_router};

mod standard_run {
    #[mads_common::routes]
    pub trait RoutedRoutes {
        #[mads_common::get("/standard-run-health")]
        async fn health(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [RoutedRoutes])]
    pub struct RoutedController;

    impl RoutedRoutes for RoutedController {
        async fn health(&self) -> &'static str {
            "healthy"
        }
    }

    #[mads_common::core::module]
    pub struct RoutedApp;
}

fn core_error(message: &str) -> Error {
    Error::new(Diagnostic::new(MADS020, "runtime test failure", message))
}

#[test]
fn runtime_errors_preserve_structured_sources() {
    let bootstrap = HttpRuntimeError::Bootstrap(core_error("catalog invalid"));
    assert!(bootstrap.to_string().contains("HTTP bootstrap failed"));
    let source = bootstrap.source().unwrap().downcast_ref::<Error>().unwrap();
    assert_eq!(source.code(), MADS020);

    let bind = HttpRuntimeError::Bind(io::Error::new(io::ErrorKind::AddrInUse, "occupied"));
    assert!(bind.to_string().contains("HTTP listener bind failed"));
    assert_eq!(
        bind.source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::AddrInUse
    );

    let combined = HttpRuntimeError::OperationAndShutdown {
        operation: Box::new(bind),
        shutdown: core_error("cleanup failed"),
    };
    assert!(combined.to_string().contains("shutdown also failed"));
    assert!(matches!(combined.source(), Some(source) if source.is::<HttpRuntimeError>()));
}

#[tokio::test]
async fn serve_router_is_available_for_raw_native_routers() {
    let application = Mads::builder().build().await.unwrap();
    let runtime = serve_router(application, mads_common::axum::Router::new(), "127.0.0.1:0");

    drop(runtime);
}

#[test]
fn run_extension_is_available_for_root_modules() {
    let runtime = Mads::run::<standard_run::RoutedApp>();

    drop(runtime);
}

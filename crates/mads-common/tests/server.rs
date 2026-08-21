//! Public HTTP runtime error contract tests.

use std::error::Error as _;
use std::io;

use mads_common::HttpRuntimeError;
use mads_common::core::{Diagnostic, Error, MADS020};

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

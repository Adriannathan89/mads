//! Offline contracts for the managed PostgreSQL pool.

#![cfg(feature = "database")]

use mads_common::{Database, DatabaseConfig, DatabaseErrorKind, diesel::prelude::*};

#[test]
fn database_pool_uses_configured_size_and_clones_share_close_state() {
    let config = DatabaseConfig::new("postgres://user:top-secret@localhost/mads")
        .unwrap()
        .with_pool_size(3)
        .unwrap();
    let database = Database::from_config(&config).unwrap();
    let clone = database.clone();

    assert_eq!(database.status().max_size(), 3);
    assert_eq!(database.status().size(), 0);
    assert_eq!(database.status().available(), 0);
    assert!(!database.status().closed());
    assert!(!format!("{database:?}").contains("localhost"));
    assert!(!format!("{database:?}").contains("top-secret"));

    clone.close();
    assert!(database.is_closed());
    assert!(database.status().closed());
    database.close();
    assert!(database.is_closed());
}

#[tokio::test]
async fn closed_pool_rejects_new_operations() {
    let database =
        Database::from_config(&DatabaseConfig::new("postgres://localhost/mads").unwrap()).unwrap();
    database.close();

    let error = database
        .run(|connection| {
            diesel::select(1_i32.into_sql::<diesel::sql_types::Integer>())
                .get_result::<i32>(connection)
        })
        .await
        .unwrap_err();
    assert_eq!(error.kind(), DatabaseErrorKind::Pool);
    assert!(std::error::Error::source(&error).is_some());
}

#[tokio::test]
async fn closed_pool_rejects_readiness_checks() {
    let database =
        Database::from_config(&DatabaseConfig::new("postgres://localhost/mads").unwrap()).unwrap();
    database.close();

    let error = database.check().await.unwrap_err();

    assert_eq!(error.kind(), DatabaseErrorKind::Pool);
}

//! Ambiguous user providers remain core graph errors, not database failures.

use mads_common::{Database, DatabaseConfig};
use mads_core::{AutoConfigurationStatus, MADS002, Mads};

#[mads_core::repository]
struct AmbiguousRepository {
    database: Database,
}

impl AmbiguousRepository {
    #[allow(dead_code)]
    fn database(&self) -> &Database {
        &self.database
    }
}

#[mads_core::provider]
fn first_database() -> Database {
    Database::from_config(&DatabaseConfig::new("postgres://localhost/first").unwrap()).unwrap()
}

#[mads_core::provider]
fn second_database() -> Database {
    Database::from_config(&DatabaseConfig::new("postgres://localhost/second").unwrap()).unwrap()
}

#[test]
fn multiple_static_database_providers_back_off_to_the_core_ambiguity_error() {
    let analysis = Mads::builder().analyze();
    assert_eq!(
        analysis.auto_configurations()[0].status(),
        AutoConfigurationStatus::Overridden
    );
    assert_eq!(analysis.diagnostics()[0].code(), MADS002);
    assert!(
        analysis
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code().as_str() != "MADS101")
    );
}

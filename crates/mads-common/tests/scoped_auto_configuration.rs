//! Rooted auto-configuration requirements follow the selected application scope.

#![cfg(all(feature = "http", feature = "jwt"))]
#![allow(missing_docs)]

use mads_common::{
    ClaimsPrincipal, MADS121,
    core::{AutoConfigurationReport, AutoConfigurationStatus, Config, Mads, Module, Result},
};

#[cfg(feature = "database")]
use mads_common::core::{ConfigBuilder, MapSource};
#[cfg(feature = "database")]
use mads_common::{Database, MADS101};

#[derive(serde::Deserialize)]
struct UnreachableClaims;

impl mads_common::PassportPrincipal for UnreachableClaims {
    fn has_role(&self, _role: &str) -> bool {
        false
    }

    fn has_permission(&self, _permission: &str) -> bool {
        false
    }
}

mod public_http {
    #[mads_common::routes]
    pub trait PublicRoutes {
        #[mads_common::get("/public")]
        async fn public(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [PublicRoutes])]
    pub struct PublicController;

    impl PublicRoutes for PublicController {
        async fn public(&self) -> &'static str {
            "public"
        }
    }

    #[mads_common::core::module]
    pub struct PublicHttpModule;
}

mod guarded_http {
    use super::*;

    #[mads_common::routes]
    #[mads_common::guard(strategy = "jwt", principal = ClaimsPrincipal<UnreachableClaims>)]
    pub trait GuardedRoutes {
        #[mads_common::get("/guarded")]
        async fn guarded(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [GuardedRoutes])]
    pub struct GuardedController;

    impl GuardedRoutes for GuardedController {
        async fn guarded(&self) -> &'static str {
            "guarded"
        }
    }

    #[mads_common::core::module]
    pub struct GuardedHttpModule;
}

#[cfg(feature = "database")]
mod database_consumers {
    use super::*;

    #[mads_common::core::repository]
    pub struct ReachableRepository {
        _database: Database,
    }

    #[mads_common::core::module]
    pub struct ReachableDatabaseModule;

    #[mads_common::core::repository]
    pub struct UnreachableRepository {
        _database: Database,
    }

    #[mads_common::core::module]
    pub struct UnreachableDatabaseModule;
}

mod roots {
    #[mads_common::core::module(imports = [super::public_http::PublicHttpModule])]
    pub struct PublicRoot;

    #[mads_common::core::module(imports = [
        super::public_http::PublicHttpModule,
        super::guarded_http::GuardedHttpModule,
    ])]
    pub struct GuardedRoot;

    #[cfg(feature = "database")]
    #[mads_common::core::module(imports = [
        super::public_http::PublicHttpModule,
        super::database_consumers::ReachableDatabaseModule,
    ])]
    pub struct DatabaseRoot;
}

async fn build_root<M: Module>(config: Config) -> Result<Mads> {
    let mut builder = Mads::builder_with_config(config);
    builder.root::<M>()?;
    builder.build().await
}

fn report<'a>(application: &'a Mads, identifier: &str) -> &'a AutoConfigurationReport {
    application
        .auto_configurations()
        .iter()
        .find(|report| report.identifier() == identifier)
        .expect("the official auto-configuration descriptor must be registered")
}

#[tokio::test]
async fn unreachable_guard_and_database_consumer_do_not_require_configuration() {
    let application = build_root::<roots::PublicRoot>(Config::empty())
        .await
        .unwrap();

    assert_eq!(
        report(&application, "mads.common.passport.jwt").status(),
        AutoConfigurationStatus::Skipped,
    );
    #[cfg(feature = "database")]
    assert_eq!(
        report(&application, "mads.common.database.diesel").status(),
        AutoConfigurationStatus::Skipped,
    );
}

#[tokio::test]
async fn reachable_guard_still_requires_jwt_configuration() {
    let error = match build_root::<roots::GuardedRoot>(Config::empty()).await {
        Ok(_) => panic!("a reachable JWT guard must require Passport configuration"),
        Err(error) => error,
    };

    assert_eq!(error.code(), MADS121);
}

#[cfg(feature = "database")]
#[tokio::test]
async fn reachable_database_consumer_still_requires_database_configuration() {
    let error = match build_root::<roots::DatabaseRoot>(Config::empty()).await {
        Ok(_) => panic!("a reachable database consumer must require database configuration"),
        Err(error) => error,
    };

    assert_eq!(error.code(), MADS101);
}

#[cfg(feature = "database")]
#[test]
fn database_requirements_use_the_core_selected_provider_slice() {
    let config = ConfigBuilder::new()
        .source(MapSource::new(
            "mads.toml",
            [
                ("database.url", "postgres://localhost/scoped"),
                ("passport.secret", "01234567890123456789012345678901"),
            ],
        ))
        .build()
        .unwrap();
    let mut builder = Mads::builder_with_config(config);
    builder.root::<roots::DatabaseRoot>().unwrap();
    let analysis = builder.analyze();
    let report = analysis
        .auto_configurations()
        .iter()
        .find(|report| report.identifier() == "mads.common.database.diesel")
        .expect("the database auto-configuration descriptor must be registered");

    assert!(analysis.is_valid());
    assert_eq!(report.status(), AutoConfigurationStatus::Active);
    assert_eq!(report.requirements().len(), 1);
    assert!(
        report.requirements()[0]
            .provider_type_name()
            .contains("ReachableRepository")
    );
    assert!(report.requirements().iter().all(|requirement| {
        !requirement
            .provider_type_name()
            .contains("UnreachableRepository")
    }));
}

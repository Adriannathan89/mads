//! Context-sensitive Passport strategy selection tests.

#![cfg(all(feature = "http", feature = "jwt"))]
#![allow(missing_docs)]

use std::any::TypeId;

use mads_common::{
    ClaimsPrincipal, JwtClaims, JwtTokenKind, MADS130, PassportContext, PassportPrincipal,
    PassportResult, PassportStrategy, PassportStrategyCatalog, PassportStrategyPreflight,
    core::{Module, Result},
};

#[derive(serde::Deserialize)]
pub struct FirstClaims;

#[derive(serde::Deserialize)]
pub struct SecondClaims;

#[derive(serde::Deserialize)]
pub struct CandidateClaims;

#[derive(serde::Deserialize)]
pub struct NoCustomClaims;

pub struct NonBuiltinPrincipal;

macro_rules! no_permissions {
    ($principal:ty) => {
        impl PassportPrincipal for $principal {
            fn has_role(&self, _role: &str) -> bool {
                false
            }

            fn has_permission(&self, _permission: &str) -> bool {
                false
            }
        }
    };
}

no_permissions!(FirstClaims);
no_permissions!(SecondClaims);
no_permissions!(CandidateClaims);
no_permissions!(NoCustomClaims);
no_permissions!(NonBuiltinPrincipal);

mod first {
    use super::*;

    #[mads_core::service]
    pub struct FirstJwtStrategy;

    #[mads_common::passport_strategy(name = "jwt")]
    impl PassportStrategy for FirstJwtStrategy {
        type Claims = FirstClaims;
        type Principal = ClaimsPrincipal<FirstClaims>;

        const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

        async fn validate(
            &self,
            _context: &PassportContext<'_>,
            _claims: &JwtClaims<Self::Claims>,
        ) -> PassportResult<Self::Principal> {
            unreachable!("preflight never executes a Passport strategy")
        }
    }

    #[mads_common::routes]
    #[mads_common::guard(strategy = "jwt", principal = ClaimsPrincipal<FirstClaims>)]
    pub trait FirstRoutes {
        #[mads_common::get("/first")]
        async fn profile(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [FirstRoutes])]
    pub struct FirstController;

    impl FirstRoutes for FirstController {
        async fn profile(&self) -> &'static str {
            "first"
        }
    }

    #[mads_common::core::module]
    pub struct FirstModule;
}

mod second {
    use super::*;

    #[mads_core::service]
    pub struct SecondJwtStrategy;

    #[mads_common::passport_strategy(name = "jwt")]
    impl PassportStrategy for SecondJwtStrategy {
        type Claims = SecondClaims;
        type Principal = ClaimsPrincipal<SecondClaims>;

        const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

        async fn validate(
            &self,
            _context: &PassportContext<'_>,
            _claims: &JwtClaims<Self::Claims>,
        ) -> PassportResult<Self::Principal> {
            unreachable!("preflight never executes a Passport strategy")
        }
    }

    #[mads_common::routes]
    #[mads_common::guard(strategy = "jwt", principal = ClaimsPrincipal<SecondClaims>)]
    pub trait SecondRoutes {
        #[mads_common::get("/second")]
        async fn profile(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [SecondRoutes])]
    pub struct SecondController;

    impl SecondRoutes for SecondController {
        async fn profile(&self) -> &'static str {
            "second"
        }
    }

    #[mads_common::core::module]
    pub struct SecondModule;
}

mod candidate_one {
    use super::*;

    #[mads_core::service]
    pub struct CandidateOneJwtStrategy;

    #[mads_common::passport_strategy(name = "jwt")]
    impl PassportStrategy for CandidateOneJwtStrategy {
        type Claims = CandidateClaims;
        type Principal = ClaimsPrincipal<CandidateClaims>;

        const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

        async fn validate(
            &self,
            _context: &PassportContext<'_>,
            _claims: &JwtClaims<Self::Claims>,
        ) -> PassportResult<Self::Principal> {
            unreachable!("preflight never executes a Passport strategy")
        }
    }

    #[mads_common::core::module]
    pub struct CandidateOneModule;
}

mod candidate_two {
    use super::*;

    #[mads_core::service]
    pub struct CandidateTwoJwtStrategy;

    #[mads_common::passport_strategy(name = "jwt")]
    impl PassportStrategy for CandidateTwoJwtStrategy {
        type Claims = CandidateClaims;
        type Principal = ClaimsPrincipal<CandidateClaims>;

        const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

        async fn validate(
            &self,
            _context: &PassportContext<'_>,
            _claims: &JwtClaims<Self::Claims>,
        ) -> PassportResult<Self::Principal> {
            unreachable!("preflight never executes a Passport strategy")
        }
    }

    #[mads_common::core::module]
    pub struct CandidateTwoModule;
}

mod two_candidates {
    use super::*;

    #[mads_common::routes]
    #[mads_common::guard(strategy = "jwt", principal = ClaimsPrincipal<CandidateClaims>)]
    pub trait CandidateRoutes {
        #[mads_common::get("/candidates")]
        async fn profile(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [CandidateRoutes])]
    pub struct CandidateController;

    impl CandidateRoutes for CandidateController {
        async fn profile(&self) -> &'static str {
            "candidates"
        }
    }

    #[mads_common::core::module(imports = [
        super::candidate_one::CandidateOneModule,
        super::candidate_two::CandidateTwoModule,
    ])]
    pub struct CandidateGuardModule;
}

mod private_strategy {
    use super::*;

    #[mads_core::service]
    struct PrivateJwtStrategy;

    #[mads_common::passport_strategy(name = "jwt")]
    impl PassportStrategy for PrivateJwtStrategy {
        type Claims = CandidateClaims;
        type Principal = NonBuiltinPrincipal;

        const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

        async fn validate(
            &self,
            _context: &PassportContext<'_>,
            _claims: &JwtClaims<Self::Claims>,
        ) -> PassportResult<Self::Principal> {
            unreachable!("preflight never executes a Passport strategy")
        }
    }

    #[mads_common::core::module]
    pub struct PrivateStrategyModule;
}

mod private_import {
    use super::*;

    #[mads_common::routes]
    #[mads_common::guard(strategy = "jwt", principal = NonBuiltinPrincipal)]
    pub trait PrivateRoutes {
        #[mads_common::get("/private")]
        async fn profile(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [PrivateRoutes])]
    pub struct PrivateController;

    impl PrivateRoutes for PrivateController {
        async fn profile(&self) -> &'static str {
            "private"
        }
    }

    #[mads_common::core::module(imports = [super::private_strategy::PrivateStrategyModule])]
    pub struct PrivateGuardModule;
}

mod transitive_strategy {
    use super::*;

    #[mads_core::service]
    pub struct TransitiveJwtStrategy;

    #[mads_common::passport_strategy(name = "jwt")]
    impl PassportStrategy for TransitiveJwtStrategy {
        type Claims = CandidateClaims;
        type Principal = NonBuiltinPrincipal;

        const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

        async fn validate(
            &self,
            _context: &PassportContext<'_>,
            _claims: &JwtClaims<Self::Claims>,
        ) -> PassportResult<Self::Principal> {
            unreachable!("preflight never executes a Passport strategy")
        }
    }

    #[mads_common::core::module]
    pub struct TransitiveStrategyModule;
}

mod transitive_import {
    #[mads_common::core::module(imports = [super::transitive_strategy::TransitiveStrategyModule])]
    pub struct MiddleModule;
}

mod transitive_guard {
    use super::*;

    #[mads_common::routes]
    #[mads_common::guard(strategy = "jwt", principal = NonBuiltinPrincipal)]
    pub trait TransitiveRoutes {
        #[mads_common::get("/transitive")]
        async fn profile(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [TransitiveRoutes])]
    pub struct TransitiveController;

    impl TransitiveRoutes for TransitiveController {
        async fn profile(&self) -> &'static str {
            "transitive"
        }
    }

    #[mads_common::core::module(imports = [super::transitive_import::MiddleModule])]
    pub struct TransitiveGuardModule;
}

mod no_custom {
    use super::*;

    #[mads_common::routes]
    #[mads_common::guard(strategy = "jwt", principal = ClaimsPrincipal<NoCustomClaims>)]
    pub trait NoCustomRoutes {
        #[mads_common::get("/builtin")]
        async fn profile(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [NoCustomRoutes])]
    pub struct NoCustomController;

    impl NoCustomRoutes for NoCustomController {
        async fn profile(&self) -> &'static str {
            "builtin"
        }
    }

    #[mads_common::core::module]
    pub struct NoCustomModule;
}

mod unimported_nested_strategy {
    use super::*;

    #[mads_common::routes]
    #[mads_common::guard(strategy = "jwt", principal = NonBuiltinPrincipal)]
    pub trait ParentRoutes {
        #[mads_common::get("/nested")]
        async fn profile(&self) -> &'static str;
    }

    #[mads_common::controller(routes = [ParentRoutes])]
    pub struct ParentController;

    impl ParentRoutes for ParentController {
        async fn profile(&self) -> &'static str {
            "nested"
        }
    }

    pub mod child {
        use super::*;

        #[mads_core::service]
        pub struct ChildJwtStrategy;

        #[mads_common::passport_strategy(name = "jwt")]
        impl PassportStrategy for ChildJwtStrategy {
            type Claims = CandidateClaims;
            type Principal = NonBuiltinPrincipal;

            const TOKEN_KIND: JwtTokenKind = JwtTokenKind::Access;

            async fn validate(
                &self,
                _context: &PassportContext<'_>,
                _claims: &JwtClaims<Self::Claims>,
            ) -> PassportResult<Self::Principal> {
                unreachable!("preflight never executes a Passport strategy")
            }
        }

        #[mads_common::core::module]
        pub struct ChildStrategyModule;
    }

    #[mads_common::core::module]
    pub struct ParentGuardModule;
}

mod roots {
    #[mads_common::core::module(imports = [super::first::FirstModule])]
    pub struct FirstRoot;

    #[mads_common::core::module(imports = [super::second::SecondModule])]
    pub struct SecondRoot;

    #[mads_common::core::module(imports = [super::two_candidates::CandidateGuardModule])]
    pub struct CandidateRoot;

    #[mads_common::core::module(imports = [super::private_import::PrivateGuardModule])]
    pub struct PrivateRoot;

    #[mads_common::core::module(imports = [super::transitive_guard::TransitiveGuardModule])]
    pub struct TransitiveRoot;

    #[mads_common::core::module(imports = [super::no_custom::NoCustomModule])]
    pub struct NoCustomRoot;

    #[mads_common::core::module(imports = [
        super::unimported_nested_strategy::ParentGuardModule,
    ])]
    pub struct NestedRoot;
}

fn preflight_for<M: Module>() -> Result<PassportStrategyPreflight<'static>> {
    let graph = mads_common::core::__private::build_module_graph::<M>()?;
    mads_common::__private::preflight_scoped(Some(&graph))
}

fn one_visible_custom_overrides_builtin() -> Result<PassportStrategyPreflight<'static>> {
    preflight_for::<roots::FirstRoot>()
}

fn two_visible_custom_candidates() -> Result<PassportStrategyPreflight<'static>> {
    preflight_for::<roots::CandidateRoot>()
}

fn same_name_in_disjoint_contexts() -> Result<()> {
    let first = preflight_for::<roots::FirstRoot>()?;
    let second = preflight_for::<roots::SecondRoot>()?;

    assert_selected_adapter::<first::FirstJwtStrategy>(&first);
    assert_selected_adapter::<second::SecondJwtStrategy>(&second);
    Ok(())
}

fn private_imported_strategy() -> Result<PassportStrategyPreflight<'static>> {
    preflight_for::<roots::PrivateRoot>()
}

fn transitively_imported_strategy() -> Result<PassportStrategyPreflight<'static>> {
    preflight_for::<roots::TransitiveRoot>()
}

fn strategy_in_an_unimported_child_module() -> Result<PassportStrategyPreflight<'static>> {
    preflight_for::<roots::NestedRoot>()
}

fn assert_selected_adapter<S>(preflight: &PassportStrategyPreflight<'_>)
where
    S: 'static,
{
    let expected = PassportStrategyCatalog::strategies()
        .into_iter()
        .find(|strategy| strategy.provider_type_id() == TypeId::of::<S>())
        .expect("the strategy descriptor must be registered")
        .adapter();
    let selected = preflight.bindings()[0].adapter();
    assert!(std::ptr::fn_addr_eq(selected, expected));
}

#[test]
fn one_visible_custom_overrides_the_builtin_adapter() {
    let preflight = one_visible_custom_overrides_builtin().unwrap();

    assert!(!preflight.bindings()[0].is_builtin());
    assert_selected_adapter::<first::FirstJwtStrategy>(&preflight);
}

#[test]
fn duplicate_custom_strategies_are_rejected_only_when_visible_to_one_guard() {
    assert_eq!(two_visible_custom_candidates().unwrap_err().code(), MADS130);
}

#[test]
fn same_name_in_disjoint_contexts_is_allowed() {
    assert!(same_name_in_disjoint_contexts().is_ok());
}

#[test]
fn private_direct_imported_strategy_is_not_visible() {
    assert_eq!(private_imported_strategy().unwrap_err().code(), MADS130);
}

#[test]
fn transitively_imported_strategy_is_not_visible() {
    assert_eq!(
        transitively_imported_strategy().unwrap_err().code(),
        MADS130
    );
}

#[test]
fn strategy_in_an_unimported_child_module_is_not_visible() {
    assert_eq!(
        strategy_in_an_unimported_child_module().unwrap_err().code(),
        MADS130
    );
}

#[test]
fn no_visible_custom_strategy_retains_the_builtin_jwt_adapter() {
    let preflight = preflight_for::<roots::NoCustomRoot>().unwrap();

    assert!(preflight.bindings()[0].is_builtin());
}

#[test]
fn rootless_scoped_preflight_retains_global_duplicate_validation() {
    assert_eq!(
        mads_common::__private::preflight_scoped(None)
            .unwrap_err()
            .code(),
        MADS130
    );
}

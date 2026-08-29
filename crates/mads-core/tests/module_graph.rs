//! Integration tests for deterministic rooted module graph construction.

use mads_core::{MADS008, ModuleNode, module};

struct UndeclaredRoot;

impl mads_core::Module for UndeclaredRoot {}

mod diamond {
    use super::module;

    pub mod shared {
        use super::module;

        #[module]
        pub struct SharedModule;
    }

    pub mod left {
        use super::{module, shared::SharedModule};

        #[module(imports = [SharedModule])]
        pub struct LeftModule;
    }

    pub mod right {
        use super::{module, shared::SharedModule};

        #[module(imports = [SharedModule])]
        pub struct RightModule;
    }

    pub mod root {
        use super::{left::LeftModule, module, right::RightModule};

        #[module(imports = [LeftModule, RightModule])]
        pub struct DiamondRoot;
    }
}

mod duplicate_import {
    use super::module;

    pub mod leaf {
        use super::module;

        #[module]
        pub struct LeafModule;
    }

    pub mod root {
        use super::{leaf::LeafModule, module};

        #[module(imports = [LeafModule, LeafModule])]
        pub struct DuplicateImportRoot;
    }
}

mod self_import {
    use super::module;

    #[module(imports = [SelfImportModule])]
    pub struct SelfImportModule;
}

mod cycle {
    use super::module;

    pub mod first {
        use super::{module, second::SecondCycleModule};

        #[module(imports = [SecondCycleModule])]
        pub struct FirstCycleModule;
    }

    pub mod second {
        use super::{first::FirstCycleModule, module};

        #[module(imports = [FirstCycleModule])]
        pub struct SecondCycleModule;
    }
}

mod namespace_collision {
    use super::module;

    pub mod shared_namespace {
        use super::module;

        #[module]
        pub struct FirstModule;

        #[module]
        pub struct SecondModule;
    }

    pub mod root {
        use super::{module, shared_namespace::FirstModule};

        #[module(imports = [FirstModule])]
        pub struct CollisionRoot;
    }
}

#[test]
fn diamond_graph_is_deterministic_and_deduplicated() {
    use diamond::{left::LeftModule, right::RightModule, root::DiamondRoot, shared::SharedModule};

    let graph = mads_core::__private::build_module_graph::<DiamondRoot>()
        .expect("diamond graph should be valid");
    assert_eq!(
        graph
            .modules()
            .iter()
            .map(ModuleNode::type_name)
            .collect::<Vec<_>>(),
        [
            std::any::type_name::<DiamondRoot>(),
            std::any::type_name::<LeftModule>(),
            std::any::type_name::<SharedModule>(),
            std::any::type_name::<RightModule>(),
        ],
    );
    assert_eq!(
        graph.root().type_name(),
        std::any::type_name::<DiamondRoot>()
    );
    assert_eq!(graph.imports().len(), 4);
    assert_eq!(
        graph.imports()[0].importer(&graph).type_name(),
        std::any::type_name::<DiamondRoot>()
    );
    assert_eq!(
        graph.imports()[0].imported(&graph).type_name(),
        std::any::type_name::<LeftModule>()
    );
}

#[test]
fn missing_root_metadata_uses_a_stable_subject() {
    let error = match mads_core::__private::build_module_graph::<UndeclaredRoot>() {
        Ok(_) => panic!("undeclared root must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code(), MADS008);
    assert!(error.to_string().contains("subject: requested module"));
}

#[test]
fn duplicate_direct_import_reports_stable_subject_and_location() {
    use duplicate_import::root::DuplicateImportRoot;

    let error = match mads_core::__private::build_module_graph::<DuplicateImportRoot>() {
        Ok(_) => panic!("duplicate direct imports must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code(), MADS008);
    let rendered = error.to_string();
    assert!(rendered.contains(std::any::type_name::<DuplicateImportRoot>()));
    assert!(rendered.contains("module_graph.rs"));
}

#[test]
fn self_import_reports_the_cycle_and_source_location() {
    use self_import::SelfImportModule;

    let error = match mads_core::__private::build_module_graph::<SelfImportModule>() {
        Ok(_) => panic!("self import must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code(), MADS008);
    let cycle = format!(
        "{} -> {}",
        std::any::type_name::<SelfImportModule>(),
        std::any::type_name::<SelfImportModule>()
    );
    let rendered = error.to_string();
    assert!(
        rendered.contains(&cycle),
        "unexpected diagnostic: {rendered}"
    );
    assert!(rendered.contains("module_graph.rs"));
}

#[test]
fn multi_module_cycle_reports_stable_chain_and_locations() {
    use cycle::{first::FirstCycleModule, second::SecondCycleModule};

    let error = match mads_core::__private::build_module_graph::<FirstCycleModule>() {
        Ok(_) => panic!("module cycle must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code(), MADS008);
    let cycle = format!(
        "{} -> {} -> {}",
        std::any::type_name::<FirstCycleModule>(),
        std::any::type_name::<SecondCycleModule>(),
        std::any::type_name::<FirstCycleModule>()
    );
    let rendered = error.to_string();
    assert!(
        rendered.contains(&cycle),
        "unexpected diagnostic: {rendered}"
    );
    assert_eq!(error.diagnostics().len(), 2);
    assert!(
        error
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.to_string().contains("module_graph.rs"))
    );
}

#[test]
fn rooted_namespace_collision_reports_an_unimported_declaration() {
    use namespace_collision::{root::CollisionRoot, shared_namespace};

    let error = match mads_core::__private::build_module_graph::<CollisionRoot>() {
        Ok(_) => panic!("a collision with an unimported declaration must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code(), MADS008);
    let rendered = error.to_string();
    assert!(rendered.contains(module_path_for::<shared_namespace::FirstModule>()));
    assert_eq!(error.diagnostics().len(), 2);
    assert!(
        error
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.to_string().contains("module_graph.rs"))
    );
}

#[test]
fn rootless_analysis_reports_complete_catalog_namespace_collisions() {
    let analysis = mads_core::Mads::builder().analyze();

    assert_eq!(analysis.diagnostics()[0].code(), MADS008);
    assert_eq!(analysis.diagnostics().len(), 2);
}

fn module_path_for<T>() -> &'static str {
    std::any::type_name::<T>()
        .rsplit_once("::")
        .map_or("", |(namespace, _)| namespace)
}

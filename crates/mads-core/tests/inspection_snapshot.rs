//! Integration tests for owned framework-neutral graph inspection snapshots.

use mads_core::{
    GraphInspectionSnapshot, Mads, ProviderOrigin, ProviderState, ProviderVisibility, module,
};

mod imported {
    use mads_core::module;

    #[module]
    pub struct RepositoryModule;
}

use imported::RepositoryModule;

#[derive(Clone)]
struct UserRepository;
struct UserService {
    _repository: UserRepository,
}

#[mads_core::provider]
fn user_repository() -> UserRepository {
    UserRepository
}

#[mads_core::provider]
fn user_service(repository: UserRepository) -> UserService {
    UserService {
        _repository: repository,
    }
}

#[module(imports = [RepositoryModule])]
struct AppModule;

#[test]
fn snapshot_owns_rooted_graph_metadata_after_analysis_is_dropped() {
    let snapshot = {
        let mut builder = Mads::builder();
        builder
            .root::<AppModule>()
            .expect("rooted fixture metadata should be valid");
        let analysis = builder.analyze();
        let snapshot = GraphInspectionSnapshot::from_analysis(&analysis);

        drop(analysis);
        drop(builder);
        snapshot
    };

    assert_eq!(
        snapshot.root_module(),
        Some("inspection_snapshot::AppModule")
    );
    assert_eq!(
        snapshot.modules()[0].type_name(),
        "inspection_snapshot::AppModule"
    );
    assert_eq!(snapshot.modules()[0].namespace(), "inspection_snapshot");
    assert!(
        snapshot.modules()[0]
            .location()
            .file()
            .ends_with("inspection_snapshot.rs")
    );
    assert_eq!(snapshot.imports().len(), 1);
    assert_eq!(
        snapshot.imports()[0].importer(),
        "inspection_snapshot::AppModule"
    );
    assert_eq!(
        snapshot.imports()[0].imported(),
        "inspection_snapshot::imported::RepositoryModule"
    );
    assert!(
        snapshot
            .providers()
            .windows(2)
            .all(|pair| pair[0].type_name() <= pair[1].type_name())
    );
    let service = snapshot
        .providers()
        .iter()
        .find(|provider| provider.type_name() == "inspection_snapshot::UserService")
        .expect("service provider should be retained");
    assert_eq!(service.owner(), Some("inspection_snapshot::AppModule"));
    assert_eq!(service.origin(), ProviderOrigin::Provider);
    assert_eq!(service.visibility(), ProviderVisibility::Private);
    assert_eq!(service.state(), ProviderState::Planned);
    assert!(
        service
            .location()
            .expect("declared provider should retain its location")
            .file()
            .ends_with("inspection_snapshot.rs")
    );
    let dependencies = snapshot
        .dependencies()
        .iter()
        .map(|edge| (edge.provider(), edge.dependency()))
        .collect::<Vec<_>>();
    assert!(
        dependencies.iter().any(|(provider, dependency)| {
            *provider == "inspection_snapshot::UserService"
                && *dependency == "inspection_snapshot::UserRepository"
        }),
        "dependencies: {dependencies:?}"
    );
    assert_eq!(
        snapshot.construction_order(),
        Some(
            [
                "inspection_snapshot::UserRepository".to_owned(),
                "inspection_snapshot::UserService".to_owned(),
            ]
            .as_slice()
        )
    );
    assert!(snapshot.auto_configurations().is_empty());
    assert!(snapshot.diagnostics().is_empty());
}

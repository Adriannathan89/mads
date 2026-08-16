//! Enforces the dependency direction of the core crate.

use std::collections::{HashMap, HashSet};

use cargo_metadata::{CargoOpt, DependencyKind, MetadataCommand, PackageId};

#[test]
fn core_normal_dependencies_stay_inside_the_core_boundary() {
    let workspace_manifest = format!("{}/../../Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    let metadata = MetadataCommand::new()
        .manifest_path(workspace_manifest)
        .features(CargoOpt::AllFeatures)
        .exec()
        .expect("workspace metadata should load");
    let resolve = metadata
        .resolve
        .as_ref()
        .expect("workspace metadata should include a dependency graph");
    let core = metadata
        .packages
        .iter()
        .find(|package| package.name == "mads-core")
        .expect("workspace should contain mads-core");
    let package_names: HashMap<&PackageId, &str> = metadata
        .packages
        .iter()
        .map(|package| (&package.id, package.name.as_str()))
        .collect();
    let nodes: HashMap<&PackageId, _> = resolve.nodes.iter().map(|node| (&node.id, node)).collect();

    let mut visited = HashSet::from([&core.id]);
    let mut pending = vec![&core.id];
    while let Some(package_id) = pending.pop() {
        let node = nodes
            .get(package_id)
            .expect("every package in the dependency graph should have a node");
        for dependency in &node.deps {
            let is_normal = dependency
                .dep_kinds
                .iter()
                .any(|kind| kind.kind == DependencyKind::Normal);
            if is_normal && visited.insert(&dependency.pkg) {
                pending.push(&dependency.pkg);
            }
        }
    }

    let forbidden = ["mads-common", "mads-extra", "axum", "diesel"];
    let mut violations: Vec<_> = visited
        .into_iter()
        .filter_map(|package_id| package_names.get(package_id).copied())
        .filter(|name| forbidden.contains(name))
        .collect();
    violations.sort_unstable();

    assert!(
        violations.is_empty(),
        "mads-core normal dependency graph crosses a forbidden boundary: {}",
        violations.join(", ")
    );
}

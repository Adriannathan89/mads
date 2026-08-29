//! Integration tests for deterministic descriptor discovery.

use std::any::TypeId;
use std::fs;
use std::process::Command;
use std::sync::Arc;

use mads_core::{
    Catalog, ConstructionContext, ErasedProvider, MADS001, MADS002, MADS003, Mads, Module,
    ModuleDescriptor, ProviderDescriptor, ProviderFuture, ProviderKind, ProviderVisibility,
    SourceLocation,
};

struct Alpha;
struct Duplicate;
struct DuplicateModule;
struct Missing;
struct MissingModule;
struct Zeta;

#[mads_core::module]
struct ImportedModule;

#[mads_core::module]
struct SecondImportedModule;

#[mads_core::module(imports = [SecondImportedModule, ImportedModule])]
struct AnnotatedModule;

impl Module for DuplicateModule {}
impl Module for MissingModule {}

fn alpha_type_id() -> TypeId {
    TypeId::of::<Alpha>()
}

fn zeta_type_id() -> TypeId {
    TypeId::of::<Zeta>()
}

fn duplicate_type_id() -> TypeId {
    TypeId::of::<Duplicate>()
}

fn duplicate_module_type_id() -> TypeId {
    TypeId::of::<DuplicateModule>()
}

fn alpha_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async { Ok(Arc::new(Alpha) as ErasedProvider) })
}

fn duplicate_constructor<'a>(_: &'a ConstructionContext<'a>) -> ProviderFuture<'a> {
    Box::pin(async { Ok(Arc::new(Duplicate) as ErasedProvider) })
}

inventory::submit! {
    ModuleDescriptor::new("zeta::Module", zeta_type_id, SourceLocation::new(file!(), line!(), column!()))
}

inventory::submit! {
    ModuleDescriptor::new("alpha::Module", alpha_type_id, SourceLocation::new(file!(), line!(), column!()))
}

inventory::submit! {
    ModuleDescriptor::new(
        "duplicate::Module",
        duplicate_module_type_id,
        SourceLocation::new("duplicate_module.rs", 1, 1),
    )
}

inventory::submit! {
    ModuleDescriptor::new(
        "duplicate::Module",
        duplicate_module_type_id,
        SourceLocation::new("duplicate_module.rs", 1, 1),
    )
}

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Provider,
        "alpha::Provider",
        alpha_type_id,
        &[],
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        alpha_constructor,
    )
}

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Service,
        "duplicate::First",
        duplicate_type_id,
        &[],
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        duplicate_constructor,
    )
}

inventory::submit! {
    ProviderDescriptor::new(
        ProviderKind::Repository,
        "duplicate::Second",
        duplicate_type_id,
        &[],
        ProviderVisibility::Private,
        SourceLocation::new(file!(), line!(), column!()),
        duplicate_constructor,
    )
}

#[test]
fn modules_are_sorted_by_stable_name() {
    let names: Vec<_> = Catalog::modules()
        .into_iter()
        .map(|item| item.type_name())
        .collect();

    assert_eq!(
        names,
        [
            "alpha::Module",
            concat!(module_path!(), "::AnnotatedModule"),
            concat!(module_path!(), "::ImportedModule"),
            concat!(module_path!(), "::SecondImportedModule"),
            "duplicate::Module",
            "duplicate::Module",
            "zeta::Module",
        ]
    );
}

#[test]
fn module_for_selects_the_annotated_module_descriptor() {
    fn assert_module<T: Module>() {}

    assert_module::<AnnotatedModule>();

    let descriptor = Catalog::module_for::<AnnotatedModule>()
        .expect("annotated module descriptor should be selected");

    assert_eq!(
        descriptor.type_name(),
        concat!(module_path!(), "::AnnotatedModule")
    );
    assert_eq!(descriptor.namespace(), Some(module_path!()));
    assert_eq!(descriptor.imports().len(), 2);
    assert_eq!(descriptor.imports()[0].type_name(), "SecondImportedModule");
    assert_eq!(
        descriptor.imports()[0].type_id(),
        TypeId::of::<SecondImportedModule>()
    );
    assert_eq!(descriptor.imports()[1].type_name(), "ImportedModule");
    assert_eq!(
        descriptor.imports()[1].type_id(),
        TypeId::of::<ImportedModule>()
    );
}

#[test]
fn module_macro_requires_imports_to_implement_module() {
    let consumer = tempfile::tempdir().expect("temporary consumer directory should exist");
    let source_dir = consumer.path().join("src");
    fs::create_dir(&source_dir).expect("temporary consumer source directory should exist");

    let core_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::write(
        consumer.path().join("Cargo.toml"),
        format!(
            "[package]\nname = \"module-import-bound\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nmads-core = {{ path = {core_path:?} }}\n"
        ),
    )
    .expect("temporary consumer manifest should be written");
    fs::write(
        source_dir.join("main.rs"),
        "struct NotAModule;\n\n#[mads_core::module(imports = [NotAModule])]\nstruct AppModule;\n\nfn main() {}\n",
    )
    .expect("temporary consumer source should be written");

    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--quiet", "--manifest-path"])
        .arg(consumer.path().join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", consumer.path().join("target"))
        .output()
        .expect("temporary consumer should invoke cargo check");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "consumer unexpectedly compiled");
    assert!(
        stderr.contains("NotAModule: Module") && stderr.contains("trait bound"),
        "consumer failed for an unexpected reason:\n{stderr}"
    );
}

#[test]
fn module_for_reports_repeated_exact_identities() {
    let Err(error) = Catalog::module_for::<DuplicateModule>() else {
        panic!("duplicate module declarations should be rejected");
    };

    assert_eq!(error.code(), MADS001);
}

#[test]
fn module_for_reports_missing_metadata() {
    let Err(error) = Catalog::module_for::<MissingModule>() else {
        panic!("missing module metadata should be rejected");
    };

    assert_eq!(error.code(), MADS003);
}

#[test]
fn provider_for_selects_the_matching_descriptor() {
    let descriptor = Catalog::provider_for::<Alpha>().expect("alpha provider should be selected");

    assert_eq!(descriptor.type_name(), "alpha::Provider");
    assert_eq!(descriptor.type_id(), TypeId::of::<Alpha>());
}

#[test]
fn provider_for_reports_ambiguous_descriptors() {
    let Err(error) = Catalog::provider_for::<Duplicate>() else {
        panic!("duplicates should be rejected");
    };

    assert_eq!(error.code(), MADS002);
}

#[test]
fn provider_for_reports_a_missing_descriptor() {
    let Err(error) = Catalog::provider_for::<Missing>() else {
        panic!("missing providers should be rejected");
    };

    assert_eq!(error.code(), MADS003);
}

#[tokio::test]
async fn manual_construction_rejects_ambiguous_provider_outputs() {
    let mut builder = Mads::builder();
    let Err(error) = builder.construct::<Duplicate>().await else {
        panic!("different providers for one output type must be ambiguous");
    };

    assert_eq!(error.code(), MADS002);
}

//! Release preparation scripts and stable workflow policy acceptance tests.

#![cfg(unix)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::{TempDir, tempdir};

const PACKAGES: &[&str] = &[
    "mads-core-macros",
    "mads-common-macros",
    "mads-core",
    "mads-extra",
    "mads-common",
    "mads",
    "mads-cli",
];

#[test]
fn beta_release_increments_a_matching_base_and_only_changes_versions() {
    let fixture = ReleaseFixture::new("0.7.0-beta.1");
    let readme = fs::read(fixture.root().join("README.md")).unwrap();
    let changelog = fs::read(fixture.root().join("CHANGELOG.md")).unwrap();

    let output = fixture.run("release-beta.sh", "0.7.0");

    assert_success(&output);
    fixture.assert_version("0.7.0-beta.2");
    assert_eq!(fs::read(fixture.root().join("README.md")).unwrap(), readme);
    assert_eq!(
        fs::read(fixture.root().join("CHANGELOG.md")).unwrap(),
        changelog
    );
}

#[test]
fn beta_release_starts_at_one_for_a_new_base() {
    let fixture = ReleaseFixture::new("0.7.0-beta.4");

    let output = fixture.run("release-beta.sh", "0.8.0");

    assert_success(&output);
    fixture.assert_version("0.8.0-beta.1");
}

#[test]
fn stable_release_sets_the_exact_stable_version() {
    let fixture = ReleaseFixture::new("0.7.0-beta.5");

    let output = fixture.run("release.sh", "0.7.0");

    assert_success(&output);
    fixture.assert_version("0.7.0");
}

#[test]
fn release_scripts_reject_invalid_versions_without_modifying_the_workspace() {
    let fixture = ReleaseFixture::new("0.7.0-beta.1");
    let before = fixture.version_files();

    let output = fixture.run("release.sh", "0.7");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("X.Y.Z"));
    assert_eq!(fixture.version_files(), before);
}

#[test]
fn stable_workflow_enforces_release_gates_and_dependency_order() {
    let root = workspace_root();
    let workflow = fs::read_to_string(root.join(".github/workflows/stable-publish.yml"))
        .expect("stable publication workflow should exist");

    for required in [
        "branches:\n      - main",
        "Require a stable workspace version",
        "cargo fmt --all --check",
        "cargo clippy --workspace --all-targets --all-features -- -D warnings",
        "cargo test --locked --workspace --all-features",
        "cargo test --locked --workspace --all-features --doc",
        "cargo doc --locked --workspace --all-features --no-deps",
        "cargo test --locked -p mads-cli --test database_generate_postgres -- --ignored --test-threads=1",
        "environment: stable",
        "CARGO_REGISTRY_TOKEN: ${{ secrets.CRATES_IO_TOKEN }}",
        "for attempt in {1..6}",
        "--latest",
    ] {
        assert!(
            workflow.contains(required),
            "missing workflow policy: {required}"
        );
    }
    for dependency in ["verify", "msrv", "postgres", "cli-platform"] {
        assert!(
            workflow.contains(&format!("      - {dependency}")),
            "publish must depend on {dependency}"
        );
    }
    assert!(!workflow.contains("--prerelease"));

    let mut offset = 0;
    for package in PACKAGES {
        let relative = workflow[offset..]
            .find(&format!("            {package}\n"))
            .unwrap_or_else(|| panic!("missing package {package} in publication order"));
        offset += relative + package.len();
    }
}

struct ReleaseFixture {
    root: TempDir,
}

impl ReleaseFixture {
    fn new(version: &str) -> Self {
        let root = tempdir().expect("release fixture should be created");
        write(
            &root.path().join("Cargo.toml"),
            &format!(
                "[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\"]\n\n[workspace.package]\nversion = \"{version}\"\nedition = \"2024\"\n"
            ),
        );
        for package in PACKAGES {
            let crate_root = root.path().join("crates").join(package);
            fs::create_dir_all(crate_root.join("src")).unwrap();
            write(&crate_root.join("src/lib.rs"), "");
            write(
                &crate_root.join("Cargo.toml"),
                &fixture_manifest(package, version),
            );
        }
        write(&root.path().join("README.md"), "release fixture readme\n");
        write(
            &root.path().join("CHANGELOG.md"),
            "# Changelog\n\nfixture notes\n",
        );
        let git = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root.path())
            .output()
            .expect("git should initialize the fixture");
        assert_success(&git);
        let lock = Command::new("cargo")
            .args(["generate-lockfile", "--offline"])
            .current_dir(root.path())
            .output()
            .expect("Cargo should generate the fixture lockfile");
        assert_success(&lock);
        Self { root }
    }

    fn root(&self) -> &Path {
        self.root.path()
    }

    fn run(&self, script: &str, version: &str) -> Output {
        Command::new("bash")
            .arg(workspace_root().join("script").join(script))
            .arg(version)
            .current_dir(self.root())
            .output()
            .expect("release script should execute")
    }

    fn assert_version(&self, expected: &str) {
        let root_manifest = fs::read_to_string(self.root().join("Cargo.toml")).unwrap();
        assert!(root_manifest.contains(&format!("version = \"{expected}\"")));

        for package in PACKAGES {
            let manifest =
                fs::read_to_string(self.root().join("crates").join(package).join("Cargo.toml"))
                    .unwrap();
            assert!(
                !manifest.contains("0.7.0-beta.1")
                    && !manifest.contains("0.7.0-beta.4")
                    && !manifest.contains("0.7.0-beta.5"),
                "old version remains in {package}"
            );
            for line in manifest.lines().filter(|line| line.contains("path =")) {
                assert!(
                    line.contains(&format!("version = \"={expected}\"")),
                    "internal pin was not updated in {package}: {line}"
                );
            }
        }

        let lock = fs::read_to_string(self.root().join("Cargo.lock")).unwrap();
        for package in PACKAGES {
            let record = format!("name = \"{package}\"\nversion = \"{expected}\"");
            assert!(
                lock.contains(&record),
                "lockfile missing {package} {expected}"
            );
        }
    }

    fn version_files(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut paths = vec![
            self.root().join("Cargo.toml"),
            self.root().join("Cargo.lock"),
        ];
        paths.extend(
            PACKAGES
                .iter()
                .map(|package| self.root().join("crates").join(package).join("Cargo.toml")),
        );
        paths
            .into_iter()
            .map(|path| {
                let contents = fs::read(&path).unwrap();
                (path, contents)
            })
            .collect()
    }
}

fn fixture_manifest(package: &str, version: &str) -> String {
    let dependencies = match package {
        "mads-core" => vec![("mads-core-macros", "../mads-core-macros")],
        "mads-extra" => vec![("mads-core", "../mads-core")],
        "mads-common" => vec![
            ("mads-common-macros", "../mads-common-macros"),
            ("mads-core", "../mads-core"),
        ],
        "mads" => vec![
            ("mads-common", "../mads-common"),
            ("mads-core", "../mads-core"),
            ("mads-extra", "../mads-extra"),
        ],
        "mads-cli" => vec![("mads", "../mads"), ("mads-common", "../mads-common")],
        _ => Vec::new(),
    };
    let mut manifest = format!(
        "[package]\nname = \"{package}\"\nversion.workspace = true\nedition.workspace = true\n"
    );
    if !dependencies.is_empty() {
        manifest.push_str("\n[dependencies]\n");
        for (dependency, path) in dependencies {
            manifest.push_str(&format!(
                "{dependency} = {{ path = \"{path}\", version = \"={version}\" }}\n"
            ));
        }
    }
    manifest
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("mads-cli should be inside the workspace crates directory")
        .to_path_buf()
}

fn write(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

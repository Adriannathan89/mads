use std::path::{Path, PathBuf};

use cargo_metadata::{MetadataCommand, Package, TargetKind};

use crate::{
    command::TargetSelection,
    diagnostic::{CliError, MADS200, MADS201},
};

pub(crate) struct CargoProject {
    metadata: cargo_metadata::Metadata,
    invocation_root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedPackage {
    workspace_root: PathBuf,
    package_root: PathBuf,
    manifest_path: PathBuf,
    package_id: cargo_metadata::PackageId,
    package_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedApplication {
    package: ResolvedPackage,
    binary_name: String,
    mads_version: Option<semver::Version>,
}

impl CargoProject {
    pub(crate) fn load(root: impl AsRef<Path>) -> Result<Self, CliError> {
        let invocation_root = root.as_ref().to_path_buf();
        let metadata = MetadataCommand::new()
            .current_dir(&invocation_root)
            .exec()
            .map_err(|error| {
                CliError::new(
                    MADS201,
                    "Cargo metadata could not be loaded",
                    "failed to read Cargo project metadata",
                )
                .with_source(error)
            })?;

        Ok(Self {
            metadata,
            invocation_root,
        })
    }

    pub(crate) fn resolve_package(
        &self,
        package: Option<&str>,
    ) -> Result<ResolvedPackage, CliError> {
        let selected = match package {
            Some(name) => self
                .metadata
                .workspace_packages()
                .into_iter()
                .find(|candidate| candidate.name == name)
                .ok_or_else(|| unknown_package(name, self.workspace_package_names()))?,
            None => match self.metadata.root_package() {
                Some(package) => package,
                None => self.resolve_default_package()?,
            },
        };

        Ok(self.resolved_package(selected))
    }

    pub(crate) fn resolve_application(
        &self,
        selection: &TargetSelection,
    ) -> Result<ResolvedApplication, CliError> {
        let package = self.resolve_package(selection.package.as_deref())?;
        let selected = &self.metadata[package.package_id()];
        let binary_name = resolve_binary(selected, selection.binary.as_deref())?;

        Ok(ResolvedApplication {
            mads_version: self.direct_mads_version(selected),
            package,
            binary_name,
        })
    }

    fn resolve_default_package(&self) -> Result<&Package, CliError> {
        let candidates = self.metadata.workspace_default_packages();
        match candidates.as_slice() {
            [package] => Ok(*package),
            [] => Err(CliError::new(
                MADS200,
                "Cargo application target is ambiguous",
                "no default workspace package can be selected",
            )
            .with_subject("workspace")
            .with_suggestion("pass --package <package>")),
            _ => {
                let names = sorted_package_names(candidates);
                Err(CliError::new(
                    MADS200,
                    "Cargo application target is ambiguous",
                    format!(
                        "more than one package can be selected: {}",
                        names.join(", ")
                    ),
                )
                .with_subject("workspace")
                .with_suggestion("pass --package <package>"))
            }
        }
    }

    fn resolved_package(&self, package: &Package) -> ResolvedPackage {
        let manifest_path = package.manifest_path.as_std_path().to_path_buf();
        let package_root = manifest_path
            .parent()
            .expect("Cargo manifests always have a parent directory")
            .to_path_buf();

        ResolvedPackage {
            workspace_root: self.metadata.workspace_root.as_std_path().to_path_buf(),
            package_root,
            manifest_path,
            package_id: package.id.clone(),
            package_name: package.name.clone(),
        }
    }

    fn workspace_package_names(&self) -> Vec<String> {
        sorted_package_names(self.metadata.workspace_packages())
    }

    fn direct_mads_version(&self, package: &Package) -> Option<semver::Version> {
        let node = self
            .metadata
            .resolve
            .as_ref()?
            .nodes
            .iter()
            .find(|node| node.id == package.id)?;

        node.deps.iter().find_map(|dependency| {
            self.metadata
                .packages
                .iter()
                .find(|candidate| candidate.id == dependency.pkg && candidate.name == "mads")
                .map(|candidate| candidate.version.clone())
        })
    }
}

impl ResolvedPackage {
    pub(crate) fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub(crate) fn package_root(&self) -> &Path {
        &self.package_root
    }

    pub(crate) fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub(crate) fn package_id(&self) -> &cargo_metadata::PackageId {
        &self.package_id
    }

    pub(crate) fn package_name(&self) -> &str {
        &self.package_name
    }
}

impl ResolvedApplication {
    pub(crate) fn package(&self) -> &ResolvedPackage {
        &self.package
    }

    pub(crate) fn binary_name(&self) -> &str {
        &self.binary_name
    }

    pub(crate) fn mads_version(&self) -> Option<&semver::Version> {
        self.mads_version.as_ref()
    }
}

fn unknown_package(name: &str, package_names: Vec<String>) -> CliError {
    let available = if package_names.is_empty() {
        "the workspace contains no packages".to_owned()
    } else {
        format!("available packages: {}", package_names.join(", "))
    };

    CliError::new(
        MADS200,
        "Cargo application target is ambiguous",
        format!("no workspace package is named `{name}`; {available}"),
    )
    .with_subject(format!("package `{name}`"))
    .with_suggestion("pass --package <package>")
}

fn resolve_binary(package: &Package, binary: Option<&str>) -> Result<String, CliError> {
    let binaries = package
        .targets
        .iter()
        .filter(|target| target.kind.contains(&TargetKind::Bin))
        .collect::<Vec<_>>();

    if let Some(binary) = binary {
        return binaries
            .iter()
            .find(|target| target.name == binary)
            .map(|target| target.name.clone())
            .ok_or_else(|| {
                CliError::new(
                    MADS200,
                    "Cargo application target is ambiguous",
                    format!(
                        "package `{}` has no binary target named `{binary}`",
                        package.name
                    ),
                )
                .with_subject(format!("package `{}`", package.name))
                .with_suggestion("pass --bin <binary>")
            });
    }

    if let Some(default_run) = &package.default_run {
        return binaries
            .iter()
            .find(|target| target.name == *default_run)
            .map(|target| target.name.clone())
            .ok_or_else(|| {
                CliError::new(
                    MADS200,
                    "Cargo application target is ambiguous",
                    format!(
                        "package `{}` declares default-run `{default_run}`, but no matching binary exists",
                        package.name
                    ),
                )
                .with_subject(format!("package `{}`", package.name))
                .with_suggestion("pass --bin <binary>")
            });
    }

    match binaries.as_slice() {
        [binary] => Ok(binary.name.clone()),
        [] => Err(CliError::new(
            MADS200,
            "Cargo application target is ambiguous",
            format!("package `{}` has no binary targets", package.name),
        )
        .with_subject(format!("package `{}`", package.name))
        .with_suggestion("pass --bin <binary>")),
        _ => {
            let mut names = binaries
                .iter()
                .map(|target| target.name.clone())
                .collect::<Vec<_>>();
            names.sort();
            Err(CliError::new(
                MADS200,
                "Cargo application target is ambiguous",
                format!(
                    "package `{}` has more than one binary target: {}",
                    package.name,
                    names.join(", ")
                ),
            )
            .with_subject(format!("package `{}`", package.name))
            .with_suggestion("pass --bin <binary>"))
        }
    }
}

fn sorted_package_names(packages: Vec<&Package>) -> Vec<String> {
    let mut names = packages
        .into_iter()
        .map(|package| package.name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_package_and_one_binary_need_no_selectors() {
        let project = CargoProject::load(fixture("single")).unwrap();
        let app = project
            .resolve_application(&TargetSelection::default())
            .unwrap();
        assert_eq!(app.package().package_name(), "single-app");
        assert_eq!(app.binary_name(), "single-app");
    }

    #[test]
    fn default_run_wins_and_explicit_bin_overrides_it() {
        let project = CargoProject::load(fixture("multiple/api")).unwrap();
        let default = project
            .resolve_application(&TargetSelection::default())
            .unwrap();
        assert_eq!(default.binary_name(), "server");

        let worker = project
            .resolve_application(&TargetSelection {
                package: None,
                binary: Some("worker".into()),
            })
            .unwrap();
        assert_eq!(worker.binary_name(), "worker");
    }

    #[test]
    fn a_virtual_workspace_requires_package_selection_when_default_members_are_ambiguous() {
        let project = CargoProject::load(fixture("multiple")).unwrap();
        let error = project
            .resolve_application(&TargetSelection::default())
            .unwrap_err();
        assert_eq!(error.code(), MADS200);
        assert!(error.to_string().contains("--package"));
    }

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/run")
            .join(name)
    }
}

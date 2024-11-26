// working with metadata

mod error;
pub use error::{MetadataError, MetadataResult};

use cargo_metadata::{camino::Utf8PathBuf, Target};
use std::{
    ops::Deref,
    process::{Command, Output},
};

#[derive(Debug)]
pub struct MetaWrapper(cargo_metadata::Metadata);

// try from path of Cargo.toml file
impl TryFrom<&Utf8PathBuf> for MetaWrapper {
    type Error = MetadataError;
    fn try_from(value: &Utf8PathBuf) -> Result<Self, Self::Error> {
        let metadata = cargo_metadata::MetadataCommand::new()
            .manifest_path(value)
            .exec()?;
        Ok(MetaWrapper(metadata))
    }
}

impl Deref for MetaWrapper {
    type Target = cargo_metadata::Metadata;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MetaWrapper {
    pub fn new(metadata: cargo_metadata::Metadata) -> Self {
        Self(metadata)
    }
    pub fn root_package(&self) -> MetadataResult<&cargo_metadata::Package> {
        self.0
            .root_package()
            .ok_or_else(|| MetadataError::NoRootPackage)
    }

    pub fn package_name(&self) -> MetadataResult<&str> {
        Ok(self.root_package()?.name.as_str())
    }

    pub fn package_manifest(&self) -> MetadataResult<&Utf8PathBuf> {
        Ok(&self.root_package()?.manifest_path)
    }

    pub fn package_root_dir(&self) -> MetadataResult<Utf8PathBuf> {
        let manifest_path = &self.root_package()?.manifest_path;
        Ok(manifest_path
            .parent()
            .ok_or_else(|| MetadataError::ErrorManifestPathOfMetadata(manifest_path.to_owned()))?
            .to_owned())
    }

    pub fn package_src_dir(&self) -> MetadataResult<Utf8PathBuf> {
        Ok(self.package_root_dir()?.join("src"))
    }

    pub fn get_binary_target_of_root_package(&self, bin_name: &str) -> MetadataResult<&Target> {
        self.root_package()?
            .targets
            .iter()
            .find(|t| t.is_bin() && t.name == bin_name)
            .ok_or_else(|| MetadataError::BinaryNotFound(bin_name.to_owned()))
    }

    pub fn get_library_target_of_root_package(&self) -> MetadataResult<Option<&Target>> {
        Ok(self.root_package()?.targets.iter().find(|t| t.is_lib()))
    }

    pub fn get_member_manifests_of_workspace(&self) -> Vec<(String, Utf8PathBuf)> {
        self.0
            .workspace_members
            .iter()
            .filter_map(|pid| self.0.packages.iter().find(|p| p.id == *pid))
            .map(|p| (p.name.to_owned(), p.manifest_path.to_owned()))
            .collect()
    }

    pub fn run_cargo_check_for_binary_of_root_package(
        &self,
        bin_name: &str,
    ) -> MetadataResult<Output> {
        Command::new("cargo")
            .current_dir(self.package_root_dir()?)
            .arg("check")
            .arg("--bin")
            .arg(bin_name)
            .arg("--manifest-path")
            .arg(self.package_manifest()?)
            .arg("--message-format=json")
            .output()
            .map_err(MetadataError::from)
    }
}

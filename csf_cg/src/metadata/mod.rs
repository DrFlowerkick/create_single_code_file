// working with metadata

mod error;
pub use error::{MetadataError, MetadataResult};

use cargo_metadata::camino::Utf8PathBuf;
use std::process::{Command, Output};

#[derive(Debug)]
pub struct MetaWrapper(pub cargo_metadata::Metadata);

impl MetaWrapper {
    pub fn root_package(&self) -> MetadataResult<&cargo_metadata::Package> {
        self.0
            .root_package()
            .ok_or_else(|| MetadataError::NoRootPackage)
    }

    pub fn package_name(&self) -> MetadataResult<&str> {
        Ok(self.root_package()?.name.as_str())
    }

    pub fn package_manifest(&self) -> MetadataResult<Utf8PathBuf> {
        Ok(self.root_package()?.manifest_path.to_owned())
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

    pub fn get_binary_path_of_root_package(&self, bin_name: &str) -> MetadataResult<&Utf8PathBuf> {
        Ok(&self
            .root_package()?
            .targets
            .iter()
            .find(|t| t.is_bin() && t.name == bin_name)
            .ok_or_else(|| MetadataError::BinaryNotFound(bin_name.to_owned()))?
            .src_path)
    }

    pub fn run_cargo_check_for_binary_of_root_package(
        &self,
        bin_name: &str,
    ) -> MetadataResult<Output> {
        Ok(Command::new("cargo")
            .current_dir(self.package_root_dir()?)
            .arg("check")
            .arg("--bin")
            .arg(bin_name)
            .arg("--manifest-path")
            .arg(self.package_manifest()?)
            .arg("--message-format=json")
            .output()
            .map_err(MetadataError::from)?)
    }
}

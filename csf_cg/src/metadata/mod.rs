// working with metadata

mod error;
pub use error::MetadataError;

use crate::{configuration::CliInput, error::CgResult, CgData};

use cargo_metadata::camino::Utf8PathBuf;
use std::process::{Command, Output};

// Implementations for all modes
impl<O, S> CgData<O, S> {
    pub fn root_package(&self) -> CgResult<&cargo_metadata::Package> {
        self.metadata
            .root_package()
            .ok_or_else(|| MetadataError::NoRootPackage)
            .map_err(|e| e.into())
    }

    pub fn package_name(&self) -> CgResult<&str> {
        Ok(self.root_package()?.name.as_str())
    }

    pub fn package_manifest(&self) -> CgResult<Utf8PathBuf> {
        Ok(self.root_package()?.manifest_path.to_owned())
    }

    pub fn package_root_dir(&self) -> CgResult<Utf8PathBuf> {
        let manifest_path = &self.root_package()?.manifest_path;
        Ok(manifest_path
            .parent()
            .ok_or_else(|| MetadataError::ErrorManifestPathOfMetadata(manifest_path.to_owned()))?
            .to_owned())
    }

    pub fn package_src_dir(&self) -> CgResult<Utf8PathBuf> {
        Ok(self.package_root_dir()?.join("src"))
    }

    pub fn get_binary_path_of_root_package(&self, bin_name: &str) -> CgResult<&Utf8PathBuf> {
        Ok(&self
            .root_package()?
            .targets
            .iter()
            .find(|t| t.is_bin() && t.name == bin_name)
            .ok_or_else(|| MetadataError::BinaryNotFound(bin_name.to_owned()))?
            .src_path)
    }

    pub fn run_cargo_check_for_binary_of_root_package(&self, bin_name: &str) -> CgResult<Output> {
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

// implementations for CliInput
impl<O: CliInput, S> CgData<O, S> {
    pub fn input_binary_name(&self) -> CgResult<&str> {
        Ok(if self.options.input().input == "main" {
            // if main, use crate name for bin name
            self.package_name()?
        } else {
            self.options.input().input.as_str()
        })
    }
}

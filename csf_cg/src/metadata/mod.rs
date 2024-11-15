// working with metadata

mod error;
use anyhow::Context;
pub use error::MetadataError;

use crate::{
    add_context, configuration::CliInput, error::CgResult, utilities::CODINGAME_SUPPORTED_CRATES,
    CgData,
};

use cargo_metadata::camino::Utf8PathBuf;
use std::process::{Command, Output};

pub struct LocalDependencies {
    root_dir: Utf8PathBuf,
    name: String,
}

impl LocalDependencies {
    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }
    pub fn get_root_dir(&self) -> &Utf8PathBuf {
        &self.root_dir
    }
    pub fn get_lib_src_file(&self) -> Utf8PathBuf {
        self.root_dir.join("src").join("lib.rs")
    }
}

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

    pub fn analyze_dependencies_of_package(&self) -> CgResult<Vec<LocalDependencies>> {
        let mut local_dependencies: Vec<LocalDependencies> = Vec::new();
        let mut crates_io_dependencies: Vec<String> = Vec::new();
        for dep in self.root_package()?.dependencies.iter() {
            if let Some(ref local_path) = dep.path {
                self.add_to_local_dependencies(
                    local_path.to_owned(),
                    dep.name.to_owned(),
                    &mut local_dependencies,
                );
            } else {
                let dep_name = dep.name.to_owned();
                if !CODINGAME_SUPPORTED_CRATES.contains(&dep_name.as_ref()) {
                    return Err(
                        MetadataError::CodingameUnsupportedDependencyOfChallenge(dep_name).into(),
                    );
                }
                crates_io_dependencies.push(dep_name);
            }
        }
        // check local libraries
        let mut index = 0;
        while index < local_dependencies.len() {
            self.analyze_dependencies_of_local_library(
                index,
                &crates_io_dependencies,
                &mut local_dependencies,
            )?;
            index += 1;
        }

        Ok(local_dependencies)
    }

    fn analyze_dependencies_of_local_library(
        &self,
        index: usize,
        crates_io_dependencies: &Vec<String>,
        local_dependencies: &mut Vec<LocalDependencies>,
    ) -> CgResult<()> {
        // get metadata of local library
        let manifest = local_dependencies[index].get_root_dir().join("Cargo.toml");
        let metadata = cargo_metadata::MetadataCommand::new()
            .manifest_path(manifest)
            .exec()
            .context(add_context!(
                "Unexpected error while trying to load manifest of local library."
            ))
            .map_err(MetadataError::from)?;
        // check dependencies of local library
        for dep in metadata
            .root_package()
            .context(add_context!(
                "Unexpected missing root_package of dependency"
            ))
            .map_err(MetadataError::from)?
            .dependencies
            .iter()
        {
            if let Some(ref local_path) = dep.path {
                if !local_dependencies
                    .iter()
                    .any(|ld| ld.get_root_dir() == local_path)
                {
                    // push new local dependency to list of local dependencies
                    self.add_to_local_dependencies(
                        local_path.to_owned(),
                        dep.name.to_owned(),
                        local_dependencies,
                    );
                }
            } else {
                let dep_name = dep.name.to_owned();
                if !CODINGAME_SUPPORTED_CRATES.contains(&dep_name.as_str()) && !self.options.force()
                {
                    return Err(MetadataError::CodingameUnsupportedDependencyOfLocalLibrary(
                        dep_name,
                    )
                    .into());
                }
                if !crates_io_dependencies.contains(&dep_name) && !self.options.force() {
                    return Err(MetadataError::DependencyOfLocalLibraryIsNotIncludedInDependenciesOfChallenge(dep_name).into());
                }
            }
        }
        Ok(())
    }

    fn add_to_local_dependencies(
        &self,
        root_dir: Utf8PathBuf,
        name: String,
        local_dependencies: &mut Vec<LocalDependencies>,
    ) {
        if self.options.verbose() {
            println!("Found local dependency '{name}' at '{root_dir}");
        }
        local_dependencies.push(LocalDependencies { root_dir, name });
    }
}

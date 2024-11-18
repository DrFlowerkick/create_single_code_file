// Building a tree of challenge and dependencies src files

mod error;
pub use error::{ChallengeTreeError, TreeResult};

use anyhow::Context;
use cargo_metadata::camino::Utf8PathBuf;
use std::cell::RefCell;
use syn::File;

use crate::{
    add_context, configuration::CliInput, error::CgResult, metadata::MetaWrapper,
    utilities::CODINGAME_SUPPORTED_CRATES, CgData,
};

#[derive(Debug)]
pub enum NodeTyp {
    LocalPackage(LocalPackage),
    SupportedCrate(String),
    UnSupportedCrate(String),
    BinCrate(SrcFile),
    LibCrate(SrcFile),
    Module(SrcFile),
}

#[derive(Debug)]
pub struct LocalPackage {
    pub name: String,
    pub path: Utf8PathBuf,
    pub metadata: Box<MetaWrapper>,
}

#[derive(Debug)]
pub struct SrcFile {
    pub name: String,
    pub path: Utf8PathBuf,
    pub crate_index: u32,
    pub syn: RefCell<File>,
}

#[derive(Debug)]
pub enum EdgeType {
    Dependency,
    Crate,
    Module,
    Uses,
}

impl TryFrom<cargo_metadata::Metadata> for LocalPackage {
    type Error = ChallengeTreeError;

    fn try_from(value: cargo_metadata::Metadata) -> Result<Self, Self::Error> {
        let metadata = Box::new(MetaWrapper(value));
        Ok(Self {
            name: metadata.package_name()?.to_owned(),
            path: metadata.package_root_dir()?,
            metadata,
        })
    }
}

// generic implementations for CgData concerning the challenge_tree
impl<O, S> CgData<O, S> {
    pub fn challenge_package(&self) -> &LocalPackage {
        if let NodeTyp::LocalPackage(ref package) = self.tree.node_weight(0.into()).unwrap() {
            return package;
        }
        unreachable!("Challenge package is created at instantiation of CgDate and should always be at index 0.");
    }
}

// implementations for CliInput
impl<O: CliInput, S> CgData<O, S> {
    pub fn input_binary_name(&self) -> CgResult<&str> {
        Ok(if self.options.input().input == "main" {
            // if main, use crate name for bin name
            self.challenge_package().name.as_str()
        } else {
            self.options.input().input.as_str()
        })
    }

    pub fn analyze_dependencies_of_package(&self) -> CgResult<Vec<LocalDependencies>> {
        let mut local_dependencies: Vec<LocalDependencies> = Vec::new();
        let mut crates_io_dependencies: Vec<String> = Vec::new();
        for dep in self
            .challenge_package()
            .metadata
            .root_package()?
            .dependencies
            .iter()
        {
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
                        ChallengeTreeError::CodingameUnsupportedDependencyOfChallenge(dep_name)
                            .into(),
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
            .map_err(ChallengeTreeError::from)?;
        // check dependencies of local library
        for dep in metadata
            .root_package()
            .context(add_context!(
                "Unexpected missing root_package of dependency"
            ))
            .map_err(ChallengeTreeError::from)?
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
                    return Err(
                        ChallengeTreeError::CodingameUnsupportedDependencyOfLocalLibrary(dep_name)
                            .into(),
                    );
                }
                if !crates_io_dependencies.contains(&dep_name) && !self.options.force() {
                    return Err(ChallengeTreeError::DependencyOfLocalLibraryIsNotIncludedInDependenciesOfChallenge(dep_name).into());
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

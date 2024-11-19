// Building a tree of challenge and dependencies src files

mod error;
pub use error::{ChallengeTreeError, TreeResult};

use anyhow::Context;
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::graph::NodeIndex;
use std::cell::RefCell;
use syn::File;

use crate::{
    add_context,
    configuration::{ChallengePlatform, CliInput},
    error::CgResult,
    metadata::MetaWrapper,
    utilities::CODINGAME_SUPPORTED_CRATES,
    CgData,
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

impl TryFrom<MetaWrapper> for LocalPackage {
    type Error = ChallengeTreeError;

    fn try_from(value: MetaWrapper) -> Result<Self, Self::Error> {
        Self::try_from(value.0)
    }
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

    pub fn iter_dependencies_of_challenge(&self) -> impl Iterator<Item = &LocalPackage> {
        self.tree
            .neighbors(0.into())
            .filter_map(|n| self.tree.node_weight(n))
            .filter_map(|n| {
                if let NodeTyp::LocalPackage(local_package) = n {
                    Some(local_package)
                } else {
                    None
                }
            })
    }

    pub fn get_local_dependency_package(
        &self,
        node: NodeIndex,
    ) -> Result<&LocalPackage, ChallengeTreeError> {
        if let NodeTyp::LocalPackage(dependency) = self
            .tree
            .node_weight(node)
            .context(add_context!("Unknown index of tree."))?
        {
            Ok(dependency)
        } else {
            return Err(ChallengeTreeError::NotLocalPackage);
        }
    }

    pub fn iter_local_packages(&self) -> impl Iterator<Item = (NodeIndex, &LocalPackage)> {
        self.tree
            .node_indices()
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| {
                if let NodeTyp::LocalPackage(local_package) = w {
                    Some((n, local_package))
                } else {
                    None
                }
            })
    }

    pub fn iter_local_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &LocalPackage)> {
        // just filter challenge package at index 0
        self.iter_local_packages().filter(|(n, _)| *n != 0.into())
    }

    pub fn iter_challenge_supported_crate_dependencies(
        &self,
    ) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.tree
            .node_indices()
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| {
                if let NodeTyp::SupportedCrate(supported_crate) = w {
                    Some((n, supported_crate.as_str()))
                } else {
                    None
                }
            })
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

    pub fn iter_supported_crates(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        match self.options.input().platform {
            ChallengePlatform::Codingame => Box::new(CODINGAME_SUPPORTED_CRATES.into_iter()),
            ChallengePlatform::Other => Box::new(
                self.options
                    .input()
                    .other_supported_crates
                    .iter()
                    .map(|c| c.as_str()),
            ),
        }
    }
}

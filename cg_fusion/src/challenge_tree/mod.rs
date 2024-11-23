// Building a tree of challenge and dependencies src files

mod error;
pub use error::{ChallengeTreeError, TreeResult};

mod visit;
pub use visit::BfsByEdgeType;

use anyhow::Context;
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::graph::NodeIndex;
use std::cell::RefCell;
use syn::File;

use crate::{add_context, configuration::CliInput, metadata::MetaWrapper, CgData};

#[derive(Debug)]
pub enum NodeTyp {
    LocalPackage(LocalPackage),
    ExternalSupportedPackage(String),
    ExternalUnsupportedPackage(String),
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

#[derive(Debug, PartialEq, Eq)]
pub enum EdgeType {
    Dependency,
    Crate,
    Module,
    Uses,
}

impl TryFrom<MetaWrapper> for LocalPackage {
    type Error = ChallengeTreeError;

    fn try_from(value: MetaWrapper) -> Result<Self, Self::Error> {
        let metadata = Box::new(value);
        Ok(Self {
            name: metadata.package_name()?.to_owned(),
            path: metadata.package_root_dir()?,
            metadata,
        })
    }
}

impl TryFrom<cargo_metadata::Metadata> for LocalPackage {
    type Error = ChallengeTreeError;

    fn try_from(value: cargo_metadata::Metadata) -> Result<Self, Self::Error> {
        Self::try_from(MetaWrapper::new(value))
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

    pub fn link_to_package(&mut self, source: NodeIndex, target: NodeIndex) {
        self.tree.add_edge(source, target, EdgeType::Dependency);
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

    fn iter_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &NodeTyp)> {
        // skip first element, which is root of tree and therefore not a dependency
        BfsByEdgeType::new(&self.tree, 0.into(), EdgeType::Dependency)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .skip(1)
    }

    pub fn iter_accepted_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.iter_dependencies().filter_map(|(n, w)| match w {
            NodeTyp::LocalPackage(local_package) => Some((n, local_package.name.as_str())),
            NodeTyp::ExternalSupportedPackage(name) => Some((n, name.as_str())),
            _ => None,
        })
    }

    pub fn iter_unsupported_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.iter_dependencies().filter_map(|(n, w)| match w {
            NodeTyp::ExternalUnsupportedPackage(name) => Some((n, name.as_str())),
            _ => None,
        })
    }

    pub fn add_bin_crate(
        &mut self,
        package_name: &str,
        bin_name: &str,
    ) -> Result<bool, ChallengeTreeError> {
        // ToDo: resume your work here
        Ok(false)
    }
}

impl<O: CliInput, S> CgData<O, S> {
    pub fn add_local_package(&mut self, source: NodeIndex, package: LocalPackage) -> NodeIndex {
        if self.options.verbose() {
            println!(
                "Found local dependency '{}' at '{}'",
                package.name, package.path
            );
        }
        let package_index = self.tree.add_node(NodeTyp::LocalPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
        package_index
    }

    pub fn add_external_supported_package(
        &mut self,
        source: NodeIndex,
        package: String,
    ) -> NodeIndex {
        if self.options.verbose() {
            println!("Found external supported dependency '{}'", package);
        }
        let package_index = self
            .tree
            .add_node(NodeTyp::ExternalSupportedPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
        package_index
    }

    pub fn add_external_unsupported_package(
        &mut self,
        source: NodeIndex,
        package: String,
    ) -> NodeIndex {
        if self.options.verbose() {
            println!("Found external unsupported dependency '{}'", package);
        }
        let package_index = self
            .tree
            .add_node(NodeTyp::ExternalUnsupportedPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
        package_index
    }
}

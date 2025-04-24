// Building a tree of challenge and dependencies src files

mod error;
mod expand;
mod fold_and_visit;
mod iterators;
mod map_impl_options;
mod navigate;
mod walkers;

pub use error::{ChallengeTreeError, TreeResult};
pub use expand::ExtSourcePath;
pub use fold_and_visit::{
    CratePathFolder, FusedDepPathFolder, RemoveSuperFolder, SetVisibilityToInherited,
    SynReferenceMapper, UpdateRelativePath, VariableReferences,
};
pub use walkers::{BfsByEdgeType, BfsWalker, PathElement, SourcePathWalker};

use crate::{configuration::CgCli, metadata::MetaWrapper};
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::stable_graph::StableDiGraph;
use syn::{Attribute, ImplItem, Item, TraitItem};

pub type ChallengeTree = StableDiGraph<NodeType, EdgeType>;

#[derive(Debug, Clone)]
pub enum NodeType {
    LocalPackage(LocalPackage),
    ExternalSupportedPackage(String),
    ExternalUnsupportedPackage(String),
    BinCrate(SrcFile),
    LibCrate(SrcFile),
    Module(SrcFile),
    SynItem(Item),
    SynImplItem(ImplItem),
    SynTraitItem(TraitItem),
}

impl NodeType {
    pub fn get_item_from_syn_item_node(&self) -> Option<&Item> {
        if let NodeType::SynItem(item) = self {
            Some(item)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalPackage {
    pub name: String,
    pub path: Utf8PathBuf,
    pub metadata: Box<MetaWrapper>,
}

impl LocalPackage {
    pub fn update_metadata(&mut self, options: &impl CgCli) -> TreeResult<()> {
        let metadata_command = options.manifest_metadata_command();
        let metadata = MetaWrapper::try_from(metadata_command)?;
        self.metadata = Box::new(metadata);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SrcFile {
    pub name: String,
    pub path: Utf8PathBuf,
    pub shebang: Option<String>,
    pub attrs: Vec<Attribute>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EdgeType {
    Dependency,
    Crate,
    Syn,
    Module,
    Implementation,
    RequiredByChallenge,
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

// Building a tree of challenge and dependencies src files

mod error;
mod expand;
mod navigate;
mod visit;

pub use error::{ChallengeTreeError, TreeResult};
pub use visit::BfsByEdgeType;

use crate::metadata::MetaWrapper;
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::{graph::Graph, Directed};
use syn::{File, ImplItem, Item, ItemUse};

pub type ChallengeTree = Graph<NodeTyp, EdgeType, Directed>;

#[derive(Debug)]
pub enum NodeTyp {
    LocalPackage(LocalPackage),
    ExternalSupportedPackage(String),
    ExternalUnsupportedPackage(String),
    BinCrate(CrateFile),
    LibCrate(CrateFile),
    SynItem(Item),
    SynImplItem(ImplItem),
}

impl NodeTyp {
    pub fn get_use_item_from_syn_item_node(&self) -> Option<&ItemUse> {
        if let NodeTyp::SynItem(Item::Use(use_item)) = self {
            Some(use_item)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct LocalPackage {
    pub name: String,
    pub path: Utf8PathBuf,
    pub metadata: Box<MetaWrapper>,
}

#[derive(Debug)]
pub struct CrateFile {
    pub name: String,
    pub path: Utf8PathBuf,
    pub syntax: File,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EdgeType {
    Dependency,
    Crate,
    Syn,
    Semantic,
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

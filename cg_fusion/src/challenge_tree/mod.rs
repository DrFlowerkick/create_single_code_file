// Building a tree of challenge and dependencies src files

mod error;
mod expand;
mod navigate;
mod visit;

pub use error::{ChallengeTreeError, TreeResult};
pub use navigate::{PathRoot, PathTarget};
pub use visit::BfsByEdgeType;

use crate::metadata::MetaWrapper;
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::stable_graph::StableDiGraph;
use syn::{Attribute, ImplItem, Item, ItemUse};

pub type ChallengeTree = StableDiGraph<NodeType, EdgeType>;

#[derive(Debug)]
pub enum NodeType {
    LocalPackage(LocalPackage),
    ExternalSupportedPackage(String),
    ExternalUnsupportedPackage(String),
    BinCrate(CrateFile),
    LibCrate(CrateFile),
    SynItem(Item),
    SynImplItem(ImplItem),
}

impl NodeType {
    pub fn get_item_from_syn_item_node(&self) -> Option<&Item> {
        if let NodeType::SynItem(item) = self {
            Some(item)
        } else {
            None
        }
    }
    // ToDo: delete this function
    pub fn get_use_item_from_syn_item_node(&self) -> Option<&ItemUse> {
        if let NodeType::SynItem(Item::Use(use_item)) = self {
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
    pub shebang: Option<String>,
    pub attrs: Vec<Attribute>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EdgeType {
    Dependency,
    Crate,
    Syn,
    Usage,
    Implementation,
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

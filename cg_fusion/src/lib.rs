// central library

pub(crate) mod challenge_tree;
pub mod configuration;
pub mod error;
pub(crate) mod metadata;
pub(crate) mod parsing;
pub mod processing;
pub(crate) mod utilities;

use std::collections::HashMap;

use challenge_tree::{ChallengeTree, LocalPackage, NodeType};
use configuration::{CargoCli, FusionCli};
use error::{CgError, CgResult};
use metadata::MetadataError;
use processing::ProcessingDependenciesState;

use petgraph::stable_graph::{NodeIndex, StableDiGraph};

// CgMode enum allows to extend cg-fusion for other user modes (e.G. a TUI) in the future
pub enum CgMode {
    Fusion(CgData<FusionCli, ProcessingDependenciesState>),
}

pub struct NoOptions;
pub struct NoCommand;

pub struct CgDataBuilder<O, M> {
    options: O,
    metadata_command: M,
}

impl Default for CgDataBuilder<NoOptions, NoCommand> {
    fn default() -> Self {
        Self::new()
    }
}

impl CgDataBuilder<NoOptions, NoCommand> {
    pub fn new() -> Self {
        Self {
            options: NoOptions,
            metadata_command: NoCommand,
        }
    }
}

impl CgDataBuilder<NoOptions, NoCommand> {
    pub fn set_options(self, options: CargoCli) -> CgDataBuilder<CargoCli, NoCommand> {
        CgDataBuilder {
            options,
            metadata_command: NoCommand,
        }
    }
}

impl CgDataBuilder<CargoCli, NoCommand> {
    pub fn set_command(self) -> CgDataBuilder<CargoCli, cargo_metadata::MetadataCommand> {
        CgDataBuilder {
            metadata_command: self.options.metadata_command(),
            options: self.options,
        }
    }
}

impl CgDataBuilder<CargoCli, cargo_metadata::MetadataCommand> {
    pub fn build(self) -> CgResult<CgMode> {
        let metadata = self.metadata_command.exec().map_err(MetadataError::from)?;
        // initialize root node with challenge metadata
        let root_node_value = NodeType::LocalPackage(LocalPackage::try_from(metadata)?);
        let mut tree: ChallengeTree = StableDiGraph::new();
        // root node should have index 0
        assert_eq!(tree.add_node(root_node_value), 0.into());
        match self.options {
            CargoCli::CgFusion(fusion_cli) => Ok(CgMode::Fusion(CgData {
                state: ProcessingDependenciesState,
                options: fusion_cli,
                tree,
                item_order: HashMap::new(),
                node_mapping: HashMap::new(),
            })),
        }
    }
}

pub struct CgData<O, S> {
    #[allow(dead_code)]
    state: S,
    options: O,
    tree: ChallengeTree,
    item_order: HashMap<NodeIndex, Vec<NodeIndex>>,
    node_mapping: HashMap<NodeIndex, NodeIndex>,
}

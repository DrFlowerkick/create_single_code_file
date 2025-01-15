// central library

pub mod analyze;
pub mod challenge_tree;
pub mod configuration;
pub mod error;
pub mod metadata;
pub mod parsing;
pub mod utilities;

use analyze::AnalyzeState;
use challenge_tree::{ChallengeTree, LocalPackage, NodeType};
use configuration::{CargoCli, FusionCli};
use error::{CgError, CgResult};
use metadata::MetadataError;

use petgraph::stable_graph::StableDiGraph;

// CgMode enum allows to extend cg-fusion for other user modes (e.G. a TUI) in the future
pub enum CgMode {
    Fusion(CgData<FusionCli, AnalyzeState>),
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
                _state: AnalyzeState,
                options: fusion_cli,
                tree,
            })),
        }
    }
}

pub struct CgData<O, S> {
    _state: S,
    options: O,
    tree: ChallengeTree,
}

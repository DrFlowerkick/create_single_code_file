// function to flatten module structure in fusion

use super::{ForgeState, ProcessingResult};
use crate::{add_context, configuration::CgCli, CgData};

use anyhow::Context;
use petgraph::stable_graph::NodeIndex;

pub struct FlattenFusionState;

impl<O: CgCli> CgData<O, FlattenFusionState> {
    pub fn flatten_fusion(mut self) -> ProcessingResult<CgData<O, ForgeState>> {
        Ok(self.set_state(ForgeState))
    }
}

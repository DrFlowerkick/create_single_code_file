// External use statements over multiple use statements and modules will
// be expanded into a full path use statement.
// E.g.:
// use std::fmt;
// mod sub_mod {
//     use super::fmt;
//     use fmt::Display;
// }
//
// will be expanded to:
//
// use std::fmt;
// mod sub_mod {
//     use std::fmt;
//     use std::fmt::Display;
// }
// This is necessary to ensure that the path of the external use statement is correct in later stages of fusion and flatten.

use super::{ProcessingCrateUseAndPathState, ProcessingResult};
use crate::{CgData, challenge_tree::NodeType, configuration::CgCli, parsing::SourcePath};

use petgraph::stable_graph::NodeIndex;
use syn::{Item, Path, PathSegment, UseTree, fold::Fold};

pub struct ProcessingExternalUseStatementsState;

impl<O: CgCli> CgData<O, ProcessingExternalUseStatementsState> {
    pub fn expand_external_use_statements(
        mut self,
    ) -> ProcessingResult<CgData<O, ProcessingCrateUseAndPathState>> {
        Ok(self.set_state(ProcessingCrateUseAndPathState))
    }
}

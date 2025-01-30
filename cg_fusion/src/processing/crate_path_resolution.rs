// use and path statements may start with crate. Since cg-fusion fuses all crates into one, the crate keyword is
// not needed. may lead to unexpected behavior. Therefore, this function removes the crate keyword from
// use and path statements.

use super::{ProcessingError, ProcessingImplBlocksState, ProcessingResult};
use crate::{configuration::CgCli, CgData};
pub struct ProcessingCrateUseAndPathState;

impl<O: CgCli> CgData<O, ProcessingCrateUseAndPathState> {
    pub fn remove_crate_keyword_from_use_and_path_statements(
        mut self,
    ) -> ProcessingResult<CgData<O, ProcessingImplBlocksState>> {
        unimplemented!();
        Ok(CgData {
            state: ProcessingImplBlocksState,
            options: self.options,
            tree: self.tree,
        })
    }
}
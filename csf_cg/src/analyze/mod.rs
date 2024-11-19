// analysis of input file for crate and library dependencies

mod error;
mod generic;
pub use error::AnalyzeError;

pub struct AnalyzeState;

use crate::{
    configuration::{AnalyzeCli, FusionCli, MergeCli},
    error::CgResult,
    merge::MergeState,
    CgData,
};

// do analyze for analyze mode
impl CgData<AnalyzeCli, AnalyzeState> {
    pub fn analyze(mut self) -> CgResult<()> {
        // force verbose output
        self.options.force_verbose();
        self.generic_analyze()?;
        Ok(())
    }
}

// do analyze for merge mode
impl CgData<MergeCli, AnalyzeState> {
    pub fn analyze(mut self) -> CgResult<CgData<MergeCli, MergeState>> {
        let _src_files = self.generic_analyze()?;
        Err(AnalyzeError::SomeAnalyzeError.into())
    }
}

// do analyze for fusion mode
impl CgData<FusionCli, AnalyzeState> {
    pub fn analyze(mut self) -> CgResult<CgData<FusionCli, MergeState>> {
        let _src_files = self.generic_analyze()?;
        Err(AnalyzeError::SomeAnalyzeError.into())
    }
}

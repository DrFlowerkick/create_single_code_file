// analysis of challenge for dependencies, modules, and use links

mod dependencies;
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

#[cfg(test)]
pub mod tests {

    use super::*;

    use crate::{configuration::CargoCli, CgDataBuilder, CgMode};

    pub fn setup_analyze_test() -> CgData<AnalyzeCli, AnalyzeState> {
        let mut analyze_options = AnalyzeCli::default();
        analyze_options.set_manifest_path("../cg_fusion_binary_test/Cargo.toml".into());

        let cg_data = match CgDataBuilder::new()
            .set_options(CargoCli::CgAnalyze(analyze_options))
            .set_command()
            .build()
            .unwrap()
        {
            CgMode::Analyze(cg_data) => cg_data,
            _ => unreachable!("It's always analyze"),
        };
        cg_data
    }
}

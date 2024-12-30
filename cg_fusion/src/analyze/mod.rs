// analysis of challenge for dependencies, modules, and use links

mod crate_src_files;
mod dependencies;
mod error;
mod impl_linking;
mod semantic_linking;
mod usage;
pub use error::AnalyzeError;

pub struct AnalyzeState;

use crate::{
    configuration::{AnalyzeCli, CliInput, FusionCli, MergeCli},
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
        self.generic_analyze()?;
        Err(AnalyzeError::SomeAnalyzeError.into())
    }
}

// do analyze for fusion mode
impl CgData<FusionCli, AnalyzeState> {
    pub fn analyze(mut self) -> CgResult<CgData<FusionCli, MergeState>> {
        self.generic_analyze()?;
        Err(AnalyzeError::SomeAnalyzeError.into())
    }
}

// generic analyze for all modes with CliInput
impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn generic_analyze(&mut self) -> CgResult<()> {
        // add dependencies to tree
        self.add_challenge_dependencies()?;
        // add crate and module src files to tree
        self.add_bin_src_files_of_challenge()?;
        self.add_lib_src_files()?;
        // expand use statements
        self.expand_and_link_use_statements()?;
        // link items, which are required for challenge
        self.link_challenge_semantic()?;

        Ok(())
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

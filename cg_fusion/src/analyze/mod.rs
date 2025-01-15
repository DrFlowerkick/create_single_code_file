// analysis of challenge for dependencies, modules, and use links

mod crate_src_files;
mod dependencies;
mod error;
mod impl_linking;
mod required_by_challenge;
mod usage;
pub use error::AnalyzeError;

pub struct AnalyzeState;
pub struct ProcessedState;

use crate::{
    configuration::{CgCli, FusionCli},
    error::CgResult,
    CgData,
};

// do analyze for fusion mode
impl CgData<FusionCli, AnalyzeState> {
    pub fn analyze(mut self) -> CgResult<CgData<FusionCli, ProcessedState>> {
        self.generic_analyze()?;
        Err(AnalyzeError::SomeAnalyzeError.into())
    }
}

// generic analyze for all modes with CgCli
// ToDo: this must be reworked
impl<O: CgCli> CgData<O, AnalyzeState> {
    pub fn generic_analyze(&mut self) -> CgResult<()> {
        // add dependencies to tree
        self.add_challenge_dependencies()?;
        // add crate and module src files to tree
        self.add_bin_src_files_of_challenge()?;
        self.add_lib_src_files()?;
        // expand use statements
        self.expand_use_statements()?;
        // link enums, structs, unions, and traits with their impl items
        self.link_impl_blocks_with_corresponding_item()?;
        // link items, which are required for challenge
        self.link_required_by_challenge()?;

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use super::*;

    use crate::{configuration::CargoCli, CgDataBuilder, CgMode};

    pub fn setup_processing_test() -> CgData<FusionCli, AnalyzeState> {
        let mut fusion_options = FusionCli::default();
        fusion_options.set_manifest_path("../cg_fusion_binary_test/Cargo.toml".into());

        let cg_data = match CgDataBuilder::new()
            .set_options(CargoCli::CgFusion(fusion_options))
            .set_command()
            .build()
            .unwrap()
        {
            CgMode::Fusion(cg_data) => cg_data,
        };
        cg_data
    }
}

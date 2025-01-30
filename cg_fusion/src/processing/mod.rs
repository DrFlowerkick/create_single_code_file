// processing of challenge and library code

mod crate_src_files;
mod dependencies;
mod error;
mod forge;
mod fuse_challenge;
mod impl_block_check;
mod impl_linking;
mod required_by_challenge;
mod usage;

pub use crate_src_files::ProcessingSrcFilesState;
pub use dependencies::ProcessingDependenciesState;
pub use error::{ProcessingError, ProcessingResult};
pub use forge::ForgeState;
pub use fuse_challenge::FuseChallengeState;
pub use impl_block_check::ProcessingImplItemDialogState;
pub use impl_linking::ProcessingImplBlocksState;
pub use required_by_challenge::ProcessingRequiredByChallengeState;
pub use usage::ProcessingUsageState;

#[cfg(test)]
pub mod tests {

    use super::*;

    use crate::{
        configuration::{CargoCli, FusionCli},
        CgData, CgDataBuilder, CgMode,
    };

    pub fn setup_processing_test(
        impl_config: bool,
    ) -> CgData<FusionCli, ProcessingDependenciesState> {
        let mut fusion_options = FusionCli::default();
        fusion_options.set_manifest_path("../cg_fusion_binary_test/Cargo.toml".into());
        if impl_config {
            fusion_options
                .set_impl_item_toml("../cg_fusion_binary_test/cg-fusion_config.toml".into());
        }

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

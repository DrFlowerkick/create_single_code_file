// processing of challenge and library code

mod crate_src_files;
mod dependencies;
mod error;
mod flatten_fusion;
mod forge;
mod fuse_challenge;
mod impl_block_check;
mod impl_linking;
mod path_minimizing;
mod required_by_challenge;
mod required_external_deps;
mod usage;

pub use crate_src_files::ProcessingSrcFilesState;
pub use dependencies::ProcessingDependenciesState;
pub use error::{ProcessingError, ProcessingResult};
pub use flatten_fusion::FlattenFusionState;
pub use forge::ForgeState;
pub use fuse_challenge::FuseChallengeState;
pub use impl_block_check::ProcessingImplItemDialogState;
pub use impl_linking::ProcessingImplBlocksState;
pub use path_minimizing::ProcessingCrateUseAndPathState;
pub use required_by_challenge::ProcessingRequiredByChallengeState;
pub use required_external_deps::ProcessingRequiredExternals;
pub use usage::ProcessingUsageState;

use crate::CgData;

impl<O, S> CgData<O, S> {
    fn set_state<N>(self, state: N) -> CgData<O, N> {
        CgData {
            state,
            options: self.options,
            tree: self.tree,
            item_order: self.item_order,
            node_mapping: self.node_mapping,
        }
    }
}

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

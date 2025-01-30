// final step of processing: forge fused challenge and library code into one binary crate

use super::{ProcessingError, ProcessingResult};
use crate::{
    add_context,
    challenge_tree::NodeType,
    configuration::CgCli,
    utilities::{CgDialog, DialogCli},
    CgData,
};

use anyhow::anyhow;
use quote::ToTokens;
use std::{fs, io::Write};
use syn::File;

pub struct ForgeState;

impl<O: CgCli> CgData<O, ForgeState> {
    pub fn forge_fused_challenge_and_library_code(mut self) -> ProcessingResult<()> {
        let Some((fusion_bin_index, src_file)) = self.get_fusion_bin_crate() else {
            return Err(anyhow!("{}", add_context!("Expected fusion src file.")).into());
        };
        let items = self.get_sorted_mod_content(fusion_bin_index)?;
        let fusion = File {
            shebang: src_file.shebang.to_owned(),
            attrs: src_file.attrs.to_owned(),
            items,
        };
        let fusion_file_name = src_file.name.to_owned();
        let fusion_string = fusion.to_token_stream().to_string();
        // dialog if fusion file already exist and no --force
        if src_file.path.is_file() && !self.options.force() {
            let dialog_handler = DialogCli::new(std::io::stdout());
            let prompt = format!("Overwrite existing fusion file '{}'?", src_file.path);
            let help = "Default is not overwriting (N).";
            let confirmation = dialog_handler.confirm(&prompt, help, false)?;
            if !confirmation {
                return Err(ProcessingError::UserCanceledDialog);
            }
        }
        let mut fusion_file = fs::File::create(&src_file.path)?;
        fusion_file.write_all(fusion_string.as_bytes())?;
        fusion_file.flush()?;

        // update cargo metadata of package and rum cargo check on package and cargo fmt on fusion file
        let fusion_file_path = self.get_fusion_file_path()?;
        let Some(NodeType::LocalPackage(challenge_package)) = self.tree.node_weight_mut(0.into())
        else {
            return Err(anyhow!("{}", add_context!("Expected challenge package.")).into());
        };
        challenge_package.update_metadata(&self.options)?;
        challenge_package
            .metadata
            .run_cargo_fmt_on_fusion_bin(&fusion_file_path)?
            .display_raw_output();
        challenge_package
            .metadata
            .run_cargo_check(&fusion_file_name)?
            .collect_cargo_check_messages()?;

        Ok(())
    }
}

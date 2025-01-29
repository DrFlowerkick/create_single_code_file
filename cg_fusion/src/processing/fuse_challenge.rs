// fuse all item required by challenge into a new binary crate in challenge tree

// Example snippet of how to place items inside a mode item
/*
let mut inline_mod = item_mod.to_owned();
let inline_items: Vec<Item> = self
    .iter_syn_item_neighbors(item_mod_index)
    .map(|(_, i)| i.to_owned())
    .collect();
inline_mod.content = Some((Brace::default(), inline_items));
if let Some(node_weight) = self.tree.node_weight_mut(item_mod_index) {
    *node_weight = NodeType::SynItem(Item::Mod(inline_mod));
} */

use std::{fs, io::Write};

use super::{ProcessedState, ProcessingError, ProcessingResult};
use crate::{
    add_context,
    challenge_tree::NodeType,
    configuration::CgCli,
    utilities::{CgDialog, DialogCli},
    CgData,
};

use anyhow::{anyhow, Context};
use petgraph::stable_graph::NodeIndex;
use quote::ToTokens;
use syn::{token, File, Item};

pub struct FuseChallengeState;

impl<O: CgCli> CgData<O, FuseChallengeState> {
    pub fn fuse_challenge(mut self) -> ProcessingResult<CgData<O, ProcessedState>> {
        // 1. create a new binary crate in challenge package
        // 2. copy all required items to new binary crate -> Pre-Order Traversal
        // 2.1 local crates (including lib of challenge) will be added as inline mod in binary crate
        // --> all use statements of local packages must be prefixed with crate::
        // --> copy possible attributes of crate to new mod item
        // 2.2 all mods will be included as inline mods (if not already inline)
        // --> update of mod items will be done after crate tree is setup, see step 3
        // 2.3 all impl blocks will include only required items
        // --> no sub nodes of impl_items are required
        // 3. recursive update of mod / crate items to include all of their sub items in syn mod / file statement
        // --> go down to leave of tree and than upwards -> Post-Order Traversal
        // 4. save crate file with proc macro in ./challenge_crate_dir/src/bin/configured_file_name.rs
        // 4.1 run cargo fmt on saved file
        // ToDo: add option flatten: collapse as many modules into their parent module or crate, flattening module
        // structure. Collapse is possible, if no name conflict exists. This Option is useful to reduce code size.

        // create a new binary crate in challenge package
        let fusion_bin_index = self.add_fusion_bin_crate()?;

        // add challenge bin content
        let (challenge_bin_index, _) = self
            .get_challenge_bin_crate()
            .context(add_context!("Expected challenge bin crate."))?;
        self.add_required_mod_content_to_fusion(challenge_bin_index, fusion_bin_index)?;

        // add required lib crates as modules to fusion
        let required_lib_crates: Vec<NodeIndex> = self
            .iter_lib_crates()
            .filter_map(|(n, _)| self.is_required_by_challenge(n).then_some(n))
            .collect();
        for required_lib_crate in required_lib_crates {
            self.add_lib_dependency_as_mod_to_fusion(required_lib_crate, fusion_bin_index)?;
        }

        // recursive update of mod / crate items to include all of their sub items in syn mod / file statement
        self.update_required_mod_content(fusion_bin_index)?;

        // finalize fusion
        self.save_fused_bin(fusion_bin_index)?;

        Ok(CgData {
            state: ProcessedState,
            options: self.options,
            tree: self.tree,
        })
    }

    fn update_required_mod_content(&mut self, mod_index: NodeIndex) -> ProcessingResult<()> {
        // recursive tree traversal to mod without further mods
        let item_mod_indices: Vec<NodeIndex> = self
            .iter_syn_item_neighbors(mod_index)
            .filter_map(|(n, i)| match i {
                Item::Mod(_) => Some(n),
                _ => None,
            })
            .collect();
        for item_mod_index in item_mod_indices {
            self.update_required_mod_content(item_mod_index)?;
        }
        // get sorted list of mod items
        let mod_content: Vec<Item> = self.get_sorted_mod_content(mod_index)?;

        // update current mod
        if let Some(NodeType::SynItem(Item::Mod(item_mod))) = self.tree.node_weight_mut(mod_index) {
            item_mod.content = Some((token::Brace::default(), mod_content));
            item_mod.semi = None;
        }
        Ok(())
    }

    fn save_fused_bin(&mut self, fusion_bin_index: NodeIndex) -> ProcessingResult<()> {
        let Some(NodeType::BinCrate(src_file)) = self.tree.node_weight(fusion_bin_index) else {
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

#[cfg(test)]
mod tests {

    use super::super::tests::setup_processing_test;
    use super::*;
    use crate::{challenge_tree::NodeType, utilities::get_relative_path};

    #[test]
    fn test_fuse_challenge() {
        // preparation
        let mut cg_data = setup_processing_test()
            .add_challenge_dependencies()
            .unwrap()
            .add_src_files()
            .unwrap()
            .expand_use_statements()
            .unwrap()
            .link_impl_blocks_with_corresponding_item()
            .unwrap()
            .link_required_by_challenge()
            .unwrap();
        // set option to include impl config
        cg_data
            .options
            .set_impl_item_toml("../cg_fusion_binary_test/cg-fusion_config.toml".into());
        let mut cg_data = cg_data.check_impl_blocks_required_by_challenge().unwrap();

        // test fusion code step by step

        // create a new binary crate in challenge package
        let fusion_bin_index = cg_data.add_fusion_bin_crate().unwrap();
        let Some(NodeType::BinCrate(src_file)) = cg_data.tree.node_weight(fusion_bin_index) else {
            panic!("Expected BinCrate node.");
        };
        let file_path =
            get_relative_path(&cg_data.challenge_package().path, &src_file.path).unwrap();
        assert_eq!(
            format!("{file_path}"),
            r#"src\bin\fusion_of_cg_fusion_binary_test.rs"#
        );

        // add challenge bin content
        let (challenge_bin_index, _) = cg_data.get_challenge_bin_crate().unwrap();
        cg_data
            .add_required_mod_content_to_fusion(challenge_bin_index, fusion_bin_index)
            .unwrap();

        let item_names_of_fusion_bin: Vec<String> = cg_data
            .iter_syn_item_neighbors(fusion_bin_index)
            .filter_map(|(n, _)| cg_data.get_verbose_name_of_tree_node(n).ok())
            .collect();
        assert_eq!(
            item_names_of_fusion_bin,
            [
                "Action (Use)",
                "main (Fn)",
                "MapPoint (Use)",
                "Go (Use)",
                "X (Use)",
                "Y (Use)",
            ]
        );

        // add required lib crates as modules to fusion
        let required_lib_crates: Vec<NodeIndex> = cg_data
            .iter_lib_crates()
            .filter_map(|(n, _)| cg_data.is_required_by_challenge(n).then_some(n))
            .collect();
        for required_lib_crate in required_lib_crates {
            cg_data
                .add_lib_dependency_as_mod_to_fusion(required_lib_crate, fusion_bin_index)
                .unwrap();
        }
        let item_names_of_fusion_bin: Vec<String> = cg_data
            .iter_syn_item_neighbors(fusion_bin_index)
            .filter_map(|(n, _)| cg_data.get_verbose_name_of_tree_node(n).ok())
            .collect();
        assert_eq!(
            item_names_of_fusion_bin,
            [
                "my_map_two_dim (Mod)",
                "cg_fusion_lib_test (Mod)",
                "cg_fusion_binary_test (Mod)",
                "Action (Use)",
                "main (Fn)",
                "MapPoint (Use)",
                "Go (Use)",
                "X (Use)",
                "Y (Use)",
            ]
        );

        let index_of_cg_fusion_binary_test = cg_data
            .iter_syn_item_neighbors(fusion_bin_index)
            .find_map(|(n, i)| {
                if let Item::Mod(item_mod) = i {
                    if item_mod.ident.to_string() == "cg_fusion_binary_test" {
                        Some(n)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap();
        let item_names_of_cg_fusion_binary_test: Vec<String> = cg_data
            .iter_syn_item_neighbors(index_of_cg_fusion_binary_test)
            .filter_map(|(n, _)| cg_data.get_verbose_name_of_tree_node(n).ok())
            .collect();

        dbg!(&item_names_of_cg_fusion_binary_test);
        /*
        for (_, mod_item) in cg_data.iter_syn_item_neighbors(fusion_bin_index) {
            if let Item::Mod(_) = mod_item {
                println!("{}", mod_item.to_token_stream());
            }
        } */
    }
}

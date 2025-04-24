// fuse all item required by challenge into a new binary crate in challenge tree

use super::{FlattenFusionState, ProcessingResult};
use crate::{CgData, add_context, configuration::CgCli};

use anyhow::Context;
use petgraph::stable_graph::NodeIndex;

pub struct FuseChallengeState;

impl<O: CgCli> CgData<O, FuseChallengeState> {
    pub fn fuse_challenge(mut self) -> ProcessingResult<CgData<O, FlattenFusionState>> {
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

        // create a new binary crate in challenge package
        let fusion_bin_index = self.add_fusion_bin_crate()?;

        // add challenge bin content
        let (challenge_bin_index, _) = self
            .get_challenge_bin_crate()
            .context(add_context!("Expected challenge bin crate."))?;
        self.node_mapping
            .insert(challenge_bin_index, fusion_bin_index);
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

        Ok(self.set_state(FlattenFusionState))
    }
}

#[cfg(test)]
mod tests {

    use crate::parsing::ItemName;

    use super::super::tests::setup_processing_test;
    use syn::Item;

    #[test]
    fn test_fuse_challenge() {
        // preparation
        let cg_data = setup_processing_test(true)
            .add_challenge_dependencies()
            .unwrap()
            .add_src_files()
            .unwrap()
            .expand_use_statements()
            .unwrap()
            .path_minimizing_of_use_and_path_statements()
            .unwrap()
            .link_impl_blocks_with_corresponding_item()
            .unwrap()
            .link_required_by_challenge()
            .unwrap()
            .check_impl_blocks()
            .unwrap()
            .process_external_dependencies()
            .unwrap()
            // action to test
            .fuse_challenge()
            .unwrap();

        // get fusion index
        let fusion_bin_index = cg_data.get_fusion_bin_crate().unwrap().0;

        let item_names_of_fusion_bin: Vec<String> = cg_data
            .iter_syn_item_neighbors(fusion_bin_index)
            .filter_map(|(n, _)| cg_data.get_verbose_name_of_tree_node(n).ok())
            .collect();
        assert_eq!(
            item_names_of_fusion_bin,
            [
                "my_map_two_dim (Mod)",
                "cg_fusion_binary_test (Mod)",
                "Action (Use)",
                "main (Fn)",
                "MapPoint (Use)",
                "Go (Use)",
                "X (Use)",
                "Y (Use)",
            ]
        );

        let sorted_output: Vec<String> = cg_data
            .get_sorted_mod_content(fusion_bin_index)
            .unwrap()
            .iter()
            .map(|i| format!("{}", ItemName::from(i)))
            .collect();
        assert_eq!(
            sorted_output,
            [
                "Action (Use)",
                "Go (Use)",
                "X (Use)",
                "Y (Use)",
                "MapPoint (Use)",
                "main (Fn)",
                "cg_fusion_binary_test (Mod)",
                "my_map_two_dim (Mod)",
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
        assert_eq!(
            item_names_of_cg_fusion_binary_test,
            [
                "action (Mod)",
                "fmt (Use)",
                "X (Const)",
                "Y (Const)",
                "N (Const)",
                "Value (Enum)",
                "impl fmt::Display for Value",
                "Go (Struct)",
                "impl Default for Go",
                "impl Go",
                "MyMap2D (Use)",
                "Action (Use)",
            ]
        );

        let Some(Item::Mod(item_mod)) = cg_data.get_syn_item(index_of_cg_fusion_binary_test) else {
            panic!("Expected Mod Item");
        };

        let sorted_item_names_of_cg_fusion_binary_test: Vec<String> = item_mod
            .content
            .as_ref()
            .unwrap()
            .1
            .iter()
            .map(|i| format!("{}", ItemName::from(i)))
            .collect();
        assert_eq!(
            sorted_item_names_of_cg_fusion_binary_test,
            [
                "action (Mod)",
                "Action (Use)",
                "MyMap2D (Use)",
                "fmt (Use)",
                "X (Const)",
                "Y (Const)",
                "N (Const)",
                "Value (Enum)",
                "impl fmt::Display for Value",
                "Go (Struct)",
                "impl Default for Go",
                "impl Go",
            ]
        );
    }
}

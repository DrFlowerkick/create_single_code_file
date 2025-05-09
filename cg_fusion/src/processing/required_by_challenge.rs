// Linking all items, which are required by challenge

use super::{ProcessingImplItemDialogState, ProcessingResult};
use crate::{CgData, add_context, configuration::CgCli};
use anyhow::Context;
use petgraph::stable_graph::NodeIndex;
use std::collections::HashSet;
use syn::Item;

pub struct ProcessingRequiredByChallengeState;

impl<O: CgCli> CgData<O, ProcessingRequiredByChallengeState> {
    pub fn link_required_by_challenge(
        mut self,
    ) -> ProcessingResult<CgData<O, ProcessingImplItemDialogState>> {
        // initialize linking of required items with main function of challenge bin crate
        let (challenge_bin_index, _) = self.get_challenge_bin_crate().unwrap();
        let (main_index, _) = self
            .iter_syn_item_neighbors(challenge_bin_index)
            .find_map(|(n, i)| match i {
                Item::Fn(fn_item) => {
                    if fn_item.sig.ident == "main" {
                        Some((n, i.to_owned()))
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .context(add_context!("Expected main fn of challenge bin crate."))?;
        self.add_required_by_challenge_link(challenge_bin_index, main_index)?;
        // a seen cache to make sure, that every required item is only checked once for path statements
        let mut seen_check_items: HashSet<NodeIndex> = HashSet::new();
        self.add_challenge_links_for_referenced_nodes_of_item(main_index, &mut seen_check_items)?;
        Ok(self.set_state(ProcessingImplItemDialogState))
    }
}

#[cfg(test)]
mod tests {

    use super::super::tests::setup_processing_test;
    use super::*;

    #[test]
    fn test_link_required_by_challenge() {
        // preparation
        let cg_data = setup_processing_test(false)
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
            // action to test
            .link_required_by_challenge()
            .unwrap();

        // assertion
        let items_required_by_challenge: Vec<NodeIndex> = cg_data
            .iter_items_required_by_challenge()
            .map(|(n, _)| n)
            .collect();
        let mut challenge_items_ident: Vec<String> = items_required_by_challenge
            .iter()
            .map(|n| match cg_data.get_syn_module_index(*n) {
                Some(module_index) => {
                    format!(
                        "{}::{}",
                        cg_data.get_verbose_name_of_tree_node(module_index).unwrap(),
                        cg_data.get_verbose_name_of_tree_node(*n).unwrap()
                    )
                }
                _ => {
                    format!("{}", cg_data.get_verbose_name_of_tree_node(*n).unwrap())
                }
            })
            .collect();
        challenge_items_ident.sort();
        assert_eq!(
            challenge_items_ident,
            [
                "action (Mod)::Action (Struct)",
                "action (Mod)::MapPoint (Use)",
                "action (Mod)::Value (Use)",
                "action (Mod)::X (Use)",
                "action (Mod)::Y (Use)",
                "action (Mod)::impl Action",
                "action (Mod)::impl Action::set_white (Impl Fn)",
                "cg_fusion_binary_test (binary crate)::Action (Use)",
                "cg_fusion_binary_test (binary crate)::Go (Use)",
                "cg_fusion_binary_test (binary crate)::MapPoint (Use)",
                "cg_fusion_binary_test (binary crate)::X (Use)",
                "cg_fusion_binary_test (binary crate)::Y (Use)",
                "cg_fusion_binary_test (binary crate)::main (Fn)",
                "cg_fusion_binary_test (library crate)",
                "cg_fusion_binary_test (library crate)::Action (Use)",
                "cg_fusion_binary_test (library crate)::Go (Struct)",
                "cg_fusion_binary_test (library crate)::MyMap2D (Use)",
                "cg_fusion_binary_test (library crate)::N (Const)",
                "cg_fusion_binary_test (library crate)::Value (Enum)",
                "cg_fusion_binary_test (library crate)::X (Const)",
                "cg_fusion_binary_test (library crate)::Y (Const)",
                "cg_fusion_binary_test (library crate)::action (Mod)",
                "cg_fusion_binary_test (library crate)::impl Default for Go",
                "cg_fusion_binary_test (library crate)::impl Default for Go::default (Impl Fn)",
                "cg_fusion_binary_test (library crate)::impl Go",
                "cg_fusion_binary_test (library crate)::impl Go::apply_action (Impl Fn)",
                "cg_fusion_binary_test (library crate)::impl Go::new (Impl Fn)",
                "my_map_point (Mod)::MapPoint (Struct)",
                "my_map_point (Mod)::impl<constX:usize,constY:usize> MapPoint<X,Y>",
                "my_map_point (Mod)::impl<constX:usize,constY:usize> MapPoint<X,Y>::is_in_map (Impl Fn)",
                "my_map_point (Mod)::impl<constX:usize,constY:usize> MapPoint<X,Y>::new (Impl Fn)",
                "my_map_two_dim (library crate)",
                "my_map_two_dim (library crate)::MyMap2D (Struct)",
                "my_map_two_dim (library crate)::impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>",
                "my_map_two_dim (library crate)::impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>::default (Impl Fn)",
                "my_map_two_dim (library crate)::impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>",
                "my_map_two_dim (library crate)::impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::new (Impl Fn)",
                "my_map_two_dim (library crate)::my_map_point (Mod)",
            ]
        );
    }
}

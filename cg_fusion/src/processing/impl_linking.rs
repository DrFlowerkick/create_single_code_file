// Tools to link Impl Items to their corresponding struct or enum

use super::{ProcessingRequiredByChallengeState, ProcessingResult};
use crate::{challenge_tree::SynReferenceMapper, configuration::CgCli, CgData};
use petgraph::stable_graph::NodeIndex;
use std::collections::HashSet;
use syn::{visit::Visit, Item};

pub struct ProcessingImplBlocksState;

impl<O: CgCli> CgData<O, ProcessingImplBlocksState> {
    pub fn link_impl_blocks_with_corresponding_item(
        mut self,
    ) -> ProcessingResult<CgData<O, ProcessingRequiredByChallengeState>> {
        // get indices of SynItem Nodes, which contain Impl Items
        let syn_impl_indices: Vec<(NodeIndex, HashSet<NodeIndex>)> = self
            .iter_crates()
            .flat_map(|(n, _, _)| {
                self.iter_syn_items(n).filter_map(|(n, i)| {
                    if let Item::Impl(item_impl) = i {
                        // since items of impl have been cleared out, al remaining path leaf must pont from
                        // impl block definition to their targets.
                        let mut leaf_collector = SynReferenceMapper::new(&self, n);
                        leaf_collector.visit_item_impl(item_impl);
                        Some((n, leaf_collector.leaf_nodes))
                    } else {
                        None
                    }
                })
            })
            .collect();
        for (syn_impl_index, leave_nodes) in syn_impl_indices {
            for item_index in leave_nodes.iter() {
                self.add_implementation_link(*item_index, syn_impl_index)?;
            }
        }
        Ok(self.set_state(ProcessingRequiredByChallengeState))
    }
}

#[cfg(test)]
mod tests {

    use petgraph::{visit::EdgeRef, Direction};
    use syn::Item;

    use super::super::tests::setup_processing_test;
    use crate::{challenge_tree::EdgeType, parsing::ItemName};

    #[test]
    fn test_link_impl_blocks() {
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
            // action to test
            .link_impl_blocks_with_corresponding_item()
            .unwrap();

        // test impl in cg_fusion_binary_test lib crate
        let (cg_fusion_binary_test_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "cg_fusion_binary_test")
            .unwrap();
        let (enum_value_index, _) = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_index)
            .filter_map(|(n, i)| {
                ItemName::from(i)
                    .get_ident_in_name_space()
                    .map(|id| (n, id))
            })
            .find(|(_, name)| name == "Value")
            .unwrap();
        assert_eq!(
            cg_data
                .tree
                .edges_directed(enum_value_index, Direction::Outgoing)
                .filter(|e| *e.weight() == EdgeType::Implementation)
                .count(),
            1
        );
        let (struct_go_index, _) = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_index)
            .filter_map(|(n, i)| {
                ItemName::from(i)
                    .get_ident_in_name_space()
                    .map(|id| (n, id))
            })
            .find(|(_, name)| name == "Go")
            .unwrap();
        assert_eq!(
            cg_data
                .tree
                .edges_directed(struct_go_index, Direction::Outgoing)
                .filter(|e| *e.weight() == EdgeType::Implementation)
                .count(),
            2
        );
        // test impl in my_map_two_dim lib crate
        let (my_map_two_dim_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "my_map_two_dim")
            .unwrap();

        let (struct_my_map_2d_index, _) = cg_data
            .iter_syn_item_neighbors(my_map_two_dim_index)
            .filter_map(|(n, i)| {
                ItemName::from(i)
                    .get_ident_in_name_space()
                    .map(|id| (n, id))
            })
            .find(|(_, name)| name == "MyMap2D")
            .unwrap();
        assert_eq!(
            cg_data
                .tree
                .edges_directed(struct_my_map_2d_index, Direction::Outgoing)
                .filter(|e| *e.weight() == EdgeType::Implementation)
                .count(),
            2
        );

        // test impl links of impl Display for Action
        let action_mod_index = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_index)
            .find_map(|(n, i)| {
                if let Item::Mod(item_mod) = i {
                    (item_mod.ident == "action").then_some(n)
                } else {
                    None
                }
            })
            .unwrap();
        let impl_display_for_action_block_index = cg_data
            .iter_syn_item_neighbors(action_mod_index)
            .find_map(|(n, i)| {
                if let Item::Impl(item_impl) = i {
                    item_impl.trait_.is_some().then_some(n)
                } else {
                    None
                }
            })
            .unwrap();
        for node in cg_data
            .tree
            .edges_directed(impl_display_for_action_block_index, Direction::Incoming)
            .filter(|e| *e.weight() == EdgeType::Implementation)
            .map(|e| e.source())
        {
            println!("{}", cg_data.get_verbose_name_of_tree_node(node).unwrap());
        }

        assert_eq!(
            cg_data
                .tree
                .edges_directed(impl_display_for_action_block_index, Direction::Incoming)
                .filter(|e| *e.weight() == EdgeType::Implementation)
                .count(),
            2
        );
    }
}

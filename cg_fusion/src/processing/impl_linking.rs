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
                        let mut leaf_collector = SynReferenceMapper::new(&self, n);
                        if let Some((_, trait_path, _)) = item_impl.trait_.as_ref() {
                            leaf_collector.visit_path(trait_path);
                        };
                        leaf_collector.visit_type(item_impl.self_ty.as_ref());
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
        Ok(CgData {
            state: ProcessingRequiredByChallengeState,
            options: self.options,
            tree: self.tree,
        })
    }
}

#[cfg(test)]
mod tests {

    use petgraph::Direction;

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
    }
}

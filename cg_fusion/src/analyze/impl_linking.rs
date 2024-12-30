// Tools to link Impl Items to their corresponding struct or enum

use super::AnalyzeState;
use crate::{add_context, configuration::CliInput, error::CgResult, parsing::ItemExtras, CgData};
use anyhow::Context;
use petgraph::stable_graph::NodeIndex;
use syn::{Ident, Item};

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn link_impl_blocks_with_corresponding_item(&mut self) -> CgResult<()> {
        // get indices of SynItem Nodes, which contain Impl Items
        let syn_impl_indices: Vec<NodeIndex> = self
            .iter_crates()
            .flat_map(|(n, _, _)| {
                self.iter_syn_items(n).filter_map(|(n, i)| {
                    if let Item::Impl(_) = i {
                        Some(n)
                    } else {
                        None
                    }
                })
            })
            .collect();
        for syn_impl_index in syn_impl_indices {
            // get source (parent) of syn impl item
            let source_index = self
                .get_syn_item_module_index(syn_impl_index)
                .context(add_context!("Expected source index of syn item."))?;

            if self.link_impl_block_enum_struct_trait_of_module(syn_impl_index, source_index)? {
                // linked to enum or struct of same module as impl statement
                continue;
            }
            // ToDo:
            // add implementation links for enums, structs and traits imported via use statement
        }
        Ok(())
    }

    fn link_impl_block_enum_struct_trait_of_module(
        &mut self,
        syn_impl_index: NodeIndex,
        source_index: NodeIndex,
    ) -> CgResult<bool> {
        // get indices and names of SynItem enum, struct and trait Nodes
        let syn_est_items: Vec<(NodeIndex, Ident)> = self
            .iter_syn_neighbors(source_index)
            .filter_map(|(n, i)| match i {
                Item::Enum(item_enum) => Some((n, item_enum.ident.to_owned())),
                Item::Struct(item_struct) => Some((n, item_struct.ident.to_owned())),
                Item::Trait(item_trait) => Some((n, item_trait.ident.to_owned())),
                _ => None,
            })
            .collect();

        for (syn_est_index, name) in syn_est_items {
            if let Some(true) = self
                .get_syn_item(syn_impl_index)
                .map(|i| i.first_item_impl_is_ident(&name))
            {
                self.add_implementation_by_link(syn_est_index, syn_impl_index)?;
                return Ok(true);
            } else if let Some(true) = self
                .get_syn_item(syn_impl_index)
                .map(|i| i.first_trait_impl_is_ident(&name))
            {
                self.add_implementation_by_link(syn_est_index, syn_impl_index)?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {

    use petgraph::Direction;

    use super::super::tests::setup_analyze_test;
    use crate::{challenge_tree::EdgeType, parsing::ItemName};

    #[test]
    fn test_link_impl_blocks() {
        // preparation
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        cg_data.add_lib_src_files().unwrap();
        cg_data.expand_and_link_use_statements().unwrap();

        // action to test
        cg_data.link_impl_blocks_with_corresponding_item().unwrap();

        // test impl in cg_fusion_binary_test lib crate
        let (cg_fusion_binary_test_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "cg_fusion_binary_test")
            .unwrap();
        let (enum_value_index, _) = cg_data
            .iter_syn_neighbors(cg_fusion_binary_test_index)
            .filter_map(|(n, i)| ItemName::from(i).extract_name().map(|name| (n, name)))
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
            .iter_syn_neighbors(cg_fusion_binary_test_index)
            .filter_map(|(n, i)| ItemName::from(i).extract_name().map(|name| (n, name)))
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
            .iter_syn_neighbors(my_map_two_dim_index)
            .filter_map(|(n, i)| ItemName::from(i).extract_name().map(|name| (n, name)))
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

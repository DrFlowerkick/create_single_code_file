// Tools to link Impl Items to their corresponding struct or enum

// ToDo: Delete this, if planed implementation for challenge linking will not require impl linking

use super::AnalyzeState;
use crate::{
    challenge_tree::{PathElement, PathRoot},
    configuration::CliInput,
    error::CgResult,
    CgData,
};
use petgraph::stable_graph::NodeIndex;
use syn::{Item, Path, Type};

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn link_impl_blocks_with_corresponding_item(&mut self) -> CgResult<()> {
        // get indices of SynItem Nodes, which contain Impl Items
        let syn_impl_indices: Vec<(NodeIndex, Option<Path>, Path)> = self
            .iter_crates()
            .flat_map(|(n, _, _)| {
                self.iter_syn_items(n).filter_map(|(n, i)| {
                    if let Item::Impl(item_impl) = i {
                        let trait_path = if let Some((_, trait_path, _)) = item_impl.trait_.as_ref()
                        {
                            Some(trait_path.clone())
                        } else {
                            None
                        };
                        let self_ty_path = match item_impl.self_ty.as_ref() {
                            // at current state of code, we only support Path and Reference
                            Type::Path(type_path) => Some(type_path.path.clone()),
                            Type::Reference(type_ref) => {
                                if let Type::Path(type_path) = type_ref.elem.as_ref() {
                                    Some(type_path.path.clone())
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };
                        self_ty_path.map(|self_ty_path| (n, trait_path, self_ty_path))
                    } else {
                        None
                    }
                })
            })
            .collect();
        for (syn_impl_index, trait_path, impl_path) in syn_impl_indices {
            if let Some(ref tp) = trait_path {
                self.link_impl_block_by_path(syn_impl_index, tp)?;
            }
            self.link_impl_block_by_path(syn_impl_index, &impl_path)?;
        }
        Ok(())
    }

    fn link_impl_block_by_path(&mut self, syn_impl_index: NodeIndex, path: &Path) -> CgResult<()> {
        let path_target = self.get_path_leaf(syn_impl_index, path)?;
        match path_target {
            PathElement::ExternalPackage => {
                if let PathRoot::Item(item_index) = self.get_path_root(syn_impl_index, path)? {
                    self.add_implementation_by_link(item_index, syn_impl_index)?;
                }
            }
            PathElement::Item(item_index) => {
                self.add_implementation_by_link(item_index, syn_impl_index)?;
            }
            PathElement::Glob(_) | PathElement::Group | PathElement::ItemRenamed(_, _) => {
                unreachable!("Impl path cannot be glob or group or renamed item.")
            }
            PathElement::PathCouldNotBeParsed => (), // could be traits like 'Default'
        }
        Ok(())
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

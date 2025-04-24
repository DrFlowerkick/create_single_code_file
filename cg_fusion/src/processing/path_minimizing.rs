// functions to minimize use and path statements. This will although remove the crate keyword from use and path statements.
// Since cg-fusion fuses all crates into one, the crate keyword may lead to unexpected behavior.
// Therefore, this function removes the crate keyword from use and path statements while minimizing the path.

use super::{ProcessingImplBlocksState, ProcessingResult};
use crate::{
    CgData,
    challenge_tree::{CratePathFolder, NodeType},
    configuration::CgCli,
    parsing::SourcePath,
};

use petgraph::stable_graph::NodeIndex;
use syn::{Item, UseTree, fold::Fold};

pub struct ProcessingCrateUseAndPathState;

impl<O: CgCli> CgData<O, ProcessingCrateUseAndPathState> {
    pub fn path_minimizing_of_use_and_path_statements(
        mut self,
    ) -> ProcessingResult<CgData<O, ProcessingImplBlocksState>> {
        // 1. minimize use statements
        let use_item_indices: Vec<(NodeIndex, SourcePath)> = self
            .iter_crates()
            .flat_map(|(n, _, _)| self.iter_syn_items(n))
            .filter_map(|(n, i)| {
                if let syn::Item::Use(use_item) = i {
                    Some((n, SourcePath::from(use_item)))
                } else {
                    None
                }
            })
            .collect();
        for (use_item_index, use_item_path) in use_item_indices {
            let new_use_item_path =
                self.resolving_relative_source_path(use_item_index, use_item_path)?;
            let new_use_item_tree: UseTree = new_use_item_path.try_into()?;
            if let Some(NodeType::SynItem(Item::Use(use_item))) =
                self.tree.node_weight_mut(use_item_index)
            {
                use_item.tree = new_use_item_tree;
            }
        }

        // 2. minimize path statements, removing crate keyword from path statements"
        // too aggressive path minimizing breaks code, if it'is done in this simple way.
        // therefore path minimizing will only be done, if 'crate' keyword is used
        let all_syn_items: Vec<NodeIndex> = self
            .iter_crates()
            .flat_map(|(n, _, _)| self.iter_syn(n).map(|(ns, _)| ns))
            .collect();
        for syn_index in all_syn_items {
            if let Some(cloned_item) = self.clone_syn_item(syn_index) {
                let mut folder = CratePathFolder {
                    graph: &self,
                    node: syn_index,
                };
                let new_item = folder.fold_item(cloned_item);
                if let Some(NodeType::SynItem(item)) = self.tree.node_weight_mut(syn_index) {
                    *item = new_item;
                }
            }
            if let Some(cloned_impl_item) = self.clone_syn_impl_item(syn_index) {
                let mut folder = CratePathFolder {
                    graph: &self,
                    node: syn_index,
                };
                let new_impl_item = folder.fold_impl_item(cloned_impl_item);
                if let Some(NodeType::SynImplItem(impl_item)) = self.tree.node_weight_mut(syn_index)
                {
                    *impl_item = new_impl_item;
                }
            }
            if let Some(cloned_trait_item) = self.clone_syn_trait_item(syn_index) {
                let mut folder = CratePathFolder {
                    graph: &self,
                    node: syn_index,
                };
                let new_trait_item = folder.fold_trait_item(cloned_trait_item);
                if let Some(NodeType::SynTraitItem(trait_item)) =
                    self.tree.node_weight_mut(syn_index)
                {
                    *trait_item = new_trait_item;
                }
            }
        }

        Ok(self.set_state(ProcessingImplBlocksState))
    }
}

#[cfg(test)]
mod tests {

    use crate::parsing::{ItemExt, ItemName, SourcePath, ToTokensExt};
    use syn::Item;

    use super::super::tests::setup_processing_test;

    #[test]
    fn test_path_minimizing_of_use_and_path_statements() {
        // preparation
        let cg_data = setup_processing_test(false)
            .add_challenge_dependencies()
            .unwrap()
            .add_src_files()
            .unwrap()
            .expand_use_statements()
            .unwrap()
            // action to test
            .path_minimizing_of_use_and_path_statements()
            .unwrap();

        // validation
        let (cg_fusion_binary_test_lib_index, ..) = cg_data
            .iter_crates()
            .find(|(_, _, src_file)| src_file.name == "cg_fusion_binary_test")
            .unwrap();
        let use_action_index = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_lib_index)
            .find_map(|(n, i)| {
                if let Some(ident) = ItemName::from(i).get_ident_in_name_space() {
                    if ident == "Action" { Some(n) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let use_action = cg_data
            .get_syn_item(use_action_index)
            .unwrap()
            .get_item_use()
            .unwrap()
            .to_trimmed_token_string();
        assert_eq!(use_action, "use action::Action;");

        let (my_map_two_dim_index, ..) = cg_data
            .iter_crates()
            .find(|(_, _, src_file)| src_file.name == "my_map_two_dim")
            .unwrap();
        let (my_map_point_index, _) = cg_data
            .iter_syn_item_neighbors(my_map_two_dim_index)
            .find(|(_, i)| {
                if let Some(ident) = ItemName::from(*i).get_ident_in_name_space() {
                    ident == "my_map_point"
                } else {
                    false
                }
            })
            .unwrap();
        let use_compass_index = cg_data
            .iter_syn_item_neighbors(my_map_point_index)
            .find_map(|(n, i)| {
                if let Some(ident) = ItemName::from(i).get_ident_in_name_space() {
                    if ident == "Compass" { Some(n) } else { None }
                } else {
                    None
                }
            })
            .unwrap();
        let use_compass = cg_data
            .get_syn_item(use_compass_index)
            .unwrap()
            .get_item_use()
            .unwrap()
            .to_trimmed_token_string();
        assert_eq!(use_compass, "use my_compass::Compass;");

        let (_, impl_default_block_of_go) = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_lib_index)
            .find(|(_, i)| {
                if let Item::Impl(item_impl) = i {
                    match item_impl.trait_ {
                        Some((_, ref trait_path, _)) => {
                            if let Some(trait_ident) = SourcePath::from(trait_path).get_last() {
                                trait_ident == "Default"
                            } else {
                                false
                            }
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            })
            .unwrap();
        let Item::Impl(impl_default_block_of_go) = impl_default_block_of_go else {
            panic!("Expected impl block of Go.");
        };
        let impl_default_block_of_go_impl_reference =
            impl_default_block_of_go.self_ty.to_trimmed_token_string();
        assert_eq!(impl_default_block_of_go_impl_reference, "Go");

        let mod_action_index = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_lib_index)
            .find_map(|(n, i)| {
                if let Item::Mod(item_mod) = i {
                    (item_mod.ident == "action").then_some(n)
                } else {
                    None
                }
            })
            .unwrap();

        let use_fmt_index = cg_data
            .iter_syn_item_neighbors(mod_action_index)
            .find_map(|(n, i)| {
                if let Item::Use(use_item) = i {
                    if let Some(ident) = ItemName::from(use_item).get_ident_in_name_space() {
                        (ident == "fmt").then_some(n)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap();
        let use_fmt = cg_data
            .get_syn_item(use_fmt_index)
            .unwrap()
            .get_item_use()
            .unwrap()
            .to_trimmed_token_string();
        assert_eq!(use_fmt, "use super::fmt;");

        let use_fmt_display_index = cg_data
            .iter_syn_item_neighbors(mod_action_index)
            .find_map(|(n, i)| {
                if let Item::Use(use_item) = i {
                    if let Some(ident) = ItemName::from(use_item).get_ident_in_name_space() {
                        (ident == "Display").then_some(n)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap();
        let use_fmt_display = cg_data
            .get_syn_item(use_fmt_display_index)
            .unwrap()
            .get_item_use()
            .unwrap()
            .to_trimmed_token_string();
        assert_eq!(use_fmt_display, "use super::fmt::Display;");
    }
}

// functions to analyze use statements in src files

use super::AnalyzeState;
use crate::{
    add_context,
    configuration::CliInput,
    error::CgResult,
    parsing::{
        contains_use_group, get_name_of_visible_item, get_start_of_use_path, get_use_items,
        is_use_glob, replace_glob_with_ident,
    },
    CgData,
};
use anyhow::{anyhow, Context};
use petgraph::graph::NodeIndex;
use quote::ToTokens;
use syn::{Item, ItemUse, UseTree};

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn expand_use_groups(&mut self) -> CgResult<()> {
        let crate_indices = self.get_crate_indices(false)?;
        for crate_index in crate_indices {
            // get indices of SynItem Nodes, which contain UseItems with use groups
            let syn_use_indices: Vec<NodeIndex> = self
                .iter_syn_items(crate_index)
                .filter_map(|(n, i)| {
                    if let Item::Use(use_item) = i {
                        if contains_use_group(&use_item.tree) {
                            Some(n)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            for syn_use_index in syn_use_indices {
                // get source (parent) of syn use item
                let source_index = self
                    .get_syn_item_source_index(syn_use_index)
                    .context(add_context!("Expected source index of syn item."))?;
                // remove old use item from tree
                let old_use_item = self
                    .tree
                    .remove_node(syn_use_index)
                    .context(add_context!("Expected syn node to remove"))?
                    .get_use_item_from_syn_item_node()
                    .context(add_context!("Expected syn ItemUse."))?
                    .to_owned();
                if self.options.verbose() {
                    let module = self
                        .get_name_of_crate_or_module(source_index)
                        .context(add_context!("Expected crate or module name."))?;
                    println!(
                        "Expanding use group statement of module {}:\n{}",
                        module,
                        old_use_item.to_token_stream()
                    );
                }
                // expand and collect use items and add them to tree
                for new_use_tree in get_use_items(&old_use_item.tree) {
                    let mut new_use_item = old_use_item.to_owned();
                    new_use_item.tree = new_use_tree;
                    self.add_syn_item(&Item::Use(new_use_item), &"".into(), source_index)?;
                }
            }
        }
        Ok(())
    }

    pub fn expand_use_globs(&mut self) -> CgResult<()> {
        // get challenge bin and all lib crate indices in reverse order
        let crate_indices: Vec<NodeIndex> = self.get_crate_indices(true)?;
        for crate_index in crate_indices {
            self.expand_use_globs_of_crates(crate_index)?;
        }
        Ok(())
    }

    fn expand_use_globs_of_crates(&mut self, crate_index: NodeIndex) -> CgResult<()> {
        // get indices of SynItem Nodes, which contain UseItems which end on globs
        // and which do not import items from external dependencies
        let syn_use_glob_indices: Vec<NodeIndex> = self
            .iter_syn_items(crate_index)
            .filter_map(|(n, i)| {
                if let Item::Use(use_item) = i {
                    is_use_glob(&use_item.tree)
                        .and_then(|is_glob| is_glob.then_some((n, &use_item.tree)))
                } else {
                    None
                }
            })
            .filter_map(|(n, t)| {
                get_start_of_use_path(t).and_then(|name| {
                    (!self.iter_external_dependencies().any(|n| n == name)).then_some(n)
                })
            })
            .collect();
        // reverse order to start with glob use statements farthest down the tree
        for syn_use_glob_index in syn_use_glob_indices.iter().rev() {
            // get module index the glob import points to and get index of new crate, which contains the glob import
            let glob_module_index =
                self.get_module_node_index_of_glob_use(*syn_use_glob_index, crate_index)?;

            // get source (parent) of syn use item
            let source_index = self
                .get_syn_item_source_index(*syn_use_glob_index)
                .context(add_context!("Expected source index of syn item."))?;
            // remove old use item from tree
            let old_use_item = self
                .tree
                .remove_node(*syn_use_glob_index)
                .context(add_context!("Expected syn node to remove"))?
                .get_use_item_from_syn_item_node()
                .context(add_context!("Expected syn ItemUse."))?
                .to_owned();
            if self.options.verbose() {
                let module = self
                    .get_name_of_crate_or_module(source_index)
                    .context(add_context!("Expected crate or module name."))?;
                println!(
                    "Expanding use glob statement of module {}:\n{}",
                    module,
                    old_use_item.to_token_stream()
                );
            }
            // get visible items of glob import module and create new use items
            let new_use_items: Vec<ItemUse> = self
                .iter_syn_neighbors(glob_module_index)
                .filter_map(|(_, item)| get_name_of_visible_item(item))
                .filter_map(|name| replace_glob_with_ident(old_use_item.clone(), name))
                .collect();
            // add new use items to tree
            for new_use_item in new_use_items {
                self.add_syn_item(&Item::Use(new_use_item), &"".into(), source_index)?;
            }
        }
        Ok(())
    }

    fn get_module_node_index_of_glob_use(
        &self,
        use_item_node_index: NodeIndex,
        crate_index: NodeIndex,
    ) -> CgResult<NodeIndex> {
        let mut use_tree = &self
            .tree
            .node_weight(use_item_node_index)
            .context(add_context!("Expected syn item"))?
            .get_use_item_from_syn_item_node()
            .context(add_context!("Expected syn ItemUse."))?
            .tree;
        let mut current_index = self
            .get_syn_item_source_index(use_item_node_index)
            .context(add_context!("Expected source index of syn item."))?;
        // walk trough the use path
        loop {
            match use_tree {
                UseTree::Path(use_path) => {
                    let module = use_path.ident.to_string();
                    match module.as_str() {
                        "crate" => {
                            // module of current crate
                            current_index = crate_index;
                        }
                        "self" => {
                            // current module, do nothing
                        }
                        "super" => {
                            // super module
                            current_index = self
                                .get_syn_item_source_index(current_index)
                                .context(add_context!("Expected source index of syn item."))?;
                        }
                        _ => {
                            // some module, could be module of current module or local package dependency
                            if let Some((module_index, _)) = self
                                .iter_syn_neighbors(current_index)
                                .filter_map(|(n, i)| match i {
                                    Item::Mod(mod_item) => Some((n, mod_item.ident.to_string())),
                                    _ => None,
                                })
                                .find(|(_, m)| *m == module)
                            {
                                current_index = module_index;
                            } else if let Some((lib_crate_index, _)) =
                                self.iter_lib_crates().find(|(_, cf)| cf.name == module)
                            {
                                current_index = lib_crate_index;
                            } else {
                                Err(anyhow!(add_context!(format!(
                                    "Could not identify {}",
                                    module,
                                ))))?;
                            }
                        }
                    }
                    use_tree = &use_path.tree;
                }
                UseTree::Group(_) => {
                    Err(anyhow!(add_context!("Expected expanded use group.")))?;
                }
                UseTree::Glob(_) => {
                    return Ok(current_index);
                }
                UseTree::Name(_) => {
                    Err(anyhow!(add_context!(
                        "Expected UseTree::Glob, not UseTree::Name"
                    )))?;
                }
                UseTree::Rename(_) => {
                    Err(anyhow!(add_context!(
                        "Expected UseTree::Glob, not UseTree::Rename"
                    )))?;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::super::tests::setup_analyze_test;
    use super::*;

    #[test]
    fn test_expand_use_groups() {
        // preparation
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        cg_data.add_lib_src_files().unwrap();

        // number of use statements before expansion in challenge bin crate
        let (challenge_bin_crate_index, _) = cg_data.get_challenge_bin_crate().unwrap();
        assert_eq!(
            cg_data
                .iter_syn_neighbors(challenge_bin_crate_index)
                .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
                .count(),
            3
        );
        // number of use statements before expansion in cg_fusion_lib_test lib crate
        let (cg_fusion_lib_test_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "cg_fusion_lib_test")
            .unwrap();
        assert_eq!(
            cg_data
                .iter_syn_neighbors(cg_fusion_lib_test_index)
                .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
                .count(),
            5
        );

        // action to test
        cg_data.expand_use_groups().unwrap();

        // number of use statements after expansion in challenge bin crate
        let (challenge_bin_crate_index, _) = cg_data.get_challenge_bin_crate().unwrap();
        assert_eq!(
            cg_data
                .iter_syn_neighbors(challenge_bin_crate_index)
                .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
                .count(),
            5
        );
        let use_statements: Vec<String> = cg_data
            .iter_syn_neighbors(challenge_bin_crate_index)
            .filter_map(|(_, i)| match i {
                Item::Use(use_item) => Some(use_item.to_token_stream().to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(
            use_statements,
            vec![
                "use cg_fusion_binary_test :: Y ;",
                "use cg_fusion_binary_test :: X ;",
                "use cg_fusion_binary_test :: Go ;",
                "use cg_fusion_lib_test :: my_map_two_dim :: my_map_point :: * ;",
                "use cg_fusion_binary_test :: action :: Action ;",
            ]
        );
        // number of use statements after expansion in cg_fusion_lib_test lib crate
        let (cg_fusion_lib_test_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "cg_fusion_lib_test")
            .unwrap();
        assert_eq!(
            cg_data
                .iter_syn_neighbors(cg_fusion_lib_test_index)
                .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
                .count(),
            6
        );
    }

    #[test]
    fn test_expand_use_globs() {
        // preparation
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        cg_data.add_lib_src_files().unwrap();
        cg_data.expand_use_groups().unwrap();

        // action to test
        cg_data.expand_use_globs().unwrap();

        // assert use statements after expansion of globs in challenge bin crate
        let (challenge_bin_crate_index, _) = cg_data.get_challenge_bin_crate().unwrap();
        let use_statements: Vec<String> = cg_data
            .iter_syn_neighbors(challenge_bin_crate_index)
            .filter_map(|(_, i)| match i {
                Item::Use(use_item) => Some(use_item.to_token_stream().to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(
            use_statements,
            vec![
                "use cg_fusion_lib_test :: my_map_two_dim :: my_map_point :: my_compass ;",
                "use cg_fusion_lib_test :: my_map_two_dim :: my_map_point :: MapPoint ;",
                "use cg_fusion_binary_test :: Y ;",
                "use cg_fusion_binary_test :: X ;",
                "use cg_fusion_binary_test :: Go ;",
                "use cg_fusion_binary_test :: action :: Action ;",
            ]
        );

        let (my_map_two_dim_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "my_map_two_dim")
            .unwrap();
        // using iter_syn_items() here to also collect use statements of my_map_point, which is a module of my_map_two_dim
        let use_statements: Vec<String> = cg_data
            .iter_syn_items(my_map_two_dim_index)
            .filter_map(|(_, i)| match i {
                Item::Use(use_item) => Some(use_item.to_token_stream().to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(
            use_statements,
            vec![
                "use my_map_point :: my_compass :: Compass ;",
                "use my_array :: MyArray ;",
                "use self :: my_map_point :: my_compass ;",
                "use self :: my_map_point :: MapPoint ;",
                "use crate :: my_map_point :: my_compass :: Compass ;",
                "use std :: cmp :: Ordering ;",
            ]
        );

        let (cg_fusion_binary_test_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "cg_fusion_binary_test")
            .unwrap();
        // using iter_syn_items() here to also collect use statements of action, which is a module of cg_fusion_binary_test
        let use_statements: Vec<String> = cg_data
            .iter_syn_items(cg_fusion_binary_test_index)
            .filter_map(|(_, i)| match i {
                Item::Use(use_item) => Some(use_item.to_token_stream().to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(
            use_statements,
            vec![
                "use cg_fusion_lib_test :: my_map_two_dim :: my_map_point ;",
                "use cg_fusion_lib_test :: my_map_two_dim :: IsCellFreeFn ;",
                "use cg_fusion_lib_test :: my_map_two_dim :: MyMap2D ;",
                "use cg_fusion_lib_test :: my_map_two_dim :: FilterFn ;",
                "use crate :: action :: Action ;",
                "use std :: fmt ;",
                "use super :: action ;",
                "use super :: X ;",
                "use super :: Y ;",
                "use super :: Value ;",
                "use super :: Go ;",
                "use cg_fusion_lib_test :: my_map_two_dim :: my_map_point :: MapPoint ;",
            ]
        );
    }
}

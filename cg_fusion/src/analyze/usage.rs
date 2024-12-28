// functions to analyze use statements in src files

use super::{AnalyzeError, AnalyzeState};
use crate::{
    add_context,
    challenge_tree::PathTarget,
    configuration::CliInput,
    error::CgResult,
    parsing::{
        contains_use_group, extract_visibility, get_use_items, is_use_glob,
        replace_glob_with_name_or_rename_use_tree,
    },
    CgData,
};
use anyhow::{anyhow, Context};
use petgraph::graph::NodeIndex;
use quote::ToTokens;
use std::collections::{HashMap, VecDeque};
use syn::{Item, ItemUse, UseTree, Visibility};

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
                // get index of module of syn use item
                let module_index = self
                    .get_syn_item_module_index(syn_use_index)
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
                        .get_name_of_crate_or_module(module_index)
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
                    self.add_syn_item(&Item::Use(new_use_item), &"".into(), module_index)?;
                }
            }
        }
        Ok(())
    }

    pub fn expand_use_globs_and_link_use_items(&mut self) -> CgResult<()> {
        // ToDo: move max_attempts to options
        let max_attempts: u8 = 5;
        let mut use_glob_indices: VecDeque<(NodeIndex, UseTree)> = self
            .iter_crates()
            .flat_map(|(crate_index, ..)| {
                self.iter_syn_items(crate_index).filter_map(|(n, i)| {
                    if let Item::Use(item_use) = i {
                        is_use_glob(i)
                            .and_then(|is_glob| is_glob.then_some((n, item_use.tree.to_owned())))
                    } else {
                        None
                    }
                })
            })
            .collect();
        let mut use_glob_attempts: HashMap<NodeIndex, u8> = HashMap::new();
        dbg!(use_glob_indices.len());
        // expand use globs and use link local non glob items
        while let Some((use_glob_index, use_tree)) = use_glob_indices.pop_front() {
            // get index and name of module, which owns the use glob
            let use_glob_owning_module_index = self
                .get_syn_item_module_index(use_glob_index)
                .context(add_context!("Expected index of owning module of use glob."))?;
            let module = self
                .get_name_of_crate_or_module(use_glob_owning_module_index)
                .context(add_context!("Expected crate or module name."))?;

            // get module index the glob import points to and get index of new crate, which contains the glob import
            let use_glob_target_module_index =
                match self.get_path_target(use_glob_index, &use_tree)? {
                    PathTarget::ExternalPackage => continue,
                    PathTarget::Glob(gmi) => gmi,
                    PathTarget::Item(item_index) | PathTarget::ItemRenamed(item_index, _) => {
                        // Link use item
                        self.add_usage_link(use_glob_index, item_index)?;
                        continue;
                    }
                    PathTarget::PathCouldNotBeParsed => {
                        // path could not be parsed, probably because of use glob in path -> move use item to end of queue
                        if *use_glob_attempts
                            .entry(use_glob_index)
                            .and_modify(|attempts| *attempts += 1)
                            .or_insert(1)
                            >= max_attempts
                        {
                            Err(AnalyzeError::MaxAttemptsExpandingUseGlob(
                                use_tree.to_token_stream().to_string(),
                                module,
                            ))?;
                        }
                        use_glob_indices.push_back((use_glob_index, use_tree));
                        continue;
                    }
                };

            // check if module of use glob contains visible use globs
            if self
                .iter_syn_neighbors(use_glob_target_module_index)
                .any(|(n, i)| {
                    self.is_visible_for_module(n, i, use_glob_owning_module_index)
                        .is_ok_and(|vis| vis)
                        && is_use_glob(i).unwrap_or(false)
                })
            {
                // found visible use glob -> move use item to end of queue
                if *use_glob_attempts
                    .entry(use_glob_index)
                    .and_modify(|attempts| *attempts += 1)
                    .or_insert(1)
                    >= max_attempts
                {
                    Err(AnalyzeError::MaxAttemptsExpandingUseGlob(
                        use_tree.to_token_stream().to_string(),
                        module,
                    ))?;
                }
                use_glob_indices.push_back((use_glob_index, use_tree));
                continue;
            }
            // remove old use item from tree
            let old_use_item = self
                .tree
                .remove_node(use_glob_index)
                .context(add_context!("Expected syn node to remove"))?
                .get_use_item_from_syn_item_node()
                .context(add_context!("Expected syn ItemUse."))?
                .to_owned();
            if self.options.verbose() {
                println!(
                    "Expanding use glob statement of module {}:\n{}",
                    module,
                    old_use_item.to_token_stream()
                );
            }
            // get visible items of glob import module, which are not already in scope of module
            // owning the use glob, and create new use items
            let new_use_items: Vec<ItemUse> = self
                .iter_syn_neighbors(use_glob_target_module_index)
                .filter(|(n, i)| {
                    self.is_visible_for_module(*n, i, use_glob_owning_module_index)
                        .is_ok_and(|vis| vis)
                })
                .filter(|(n, _)| {
                    !self
                        .iter_syn_neighbors(use_glob_owning_module_index)
                        .any(|(m, _)| *n == m)
                })
                .filter_map(|(_, item)| match item {
                    Item::Use(use_tree) => replace_glob_with_name_or_rename_use_tree(
                        old_use_item.clone(),
                        use_tree.tree.to_owned(),
                    ),
                    _ => None,
                })
                .collect();
            // add new use items to tree
            for new_use_item in new_use_items {
                let use_tree = new_use_item.tree.to_owned();
                let new_use_item_index = self.add_syn_item(
                    &Item::Use(new_use_item),
                    &"".into(),
                    use_glob_target_module_index,
                )?;
                use_glob_indices.push_back((new_use_item_index, use_tree));
            }
        }
        Ok(())
    }

    fn is_visible_for_module(
        &self,
        item_index: NodeIndex,
        item: &Item,
        module_index: NodeIndex,
    ) -> CgResult<bool> {
        /*
        https://doc.rust-lang.org/reference/visibility-and-privacy.html
        With the notion of an item being either public or private, Rust allows item accesses in two cases:
        1. If an item is public, then it can be accessed externally from some module m if you can access all
        the item’s ancestor modules from m. You can also potentially be able to name the item through re-exports.
        2. If an item is private, it may be accessed by the current module and its descendants.
        */
        // Check module_index
        if !self.is_crate_or_module(module_index) {
            Err(anyhow!(add_context!(format!(
                "Expected crate or module at index '{:?}'.",
                module_index
            ))))?;
        }
        // check if item is descendant of module; if yes, it is visible because of rule 2 (see above)
        if self.is_item_descendant_of_or_same_module(item_index, module_index) {
            return Ok(true);
        }
        // item is not a descendant, therefore we have to analyze visibility
        if let Some(visibility) = extract_visibility(item) {
            match visibility {
                Visibility::Inherited => return Ok(false),
                Visibility::Public(_) => return Ok(true),
                Visibility::Restricted(vis_restricted) => {
                    match self.get_path_target(item_index, vis_restricted.path.as_ref())? {
                        PathTarget::ExternalPackage => return Ok(false), // only local syn items have NodeIndex to link to
                        PathTarget::Glob(_) => unreachable!("No glob in visibility path."),
                        PathTarget::ItemRenamed(_, _) => {
                            unreachable!("No rename in visibility path.")
                        }
                        PathTarget::Item(vis_path_module_index) => {
                            if self.is_item_descendant_of_or_same_module(
                                vis_path_module_index,
                                module_index,
                            ) {
                                return Ok(true);
                            }
                        }
                        PathTarget::PathCouldNotBeParsed => return Ok(false),
                    }
                }
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {

    use syn::Ident;

    use crate::parsing::ItemName;
    use crate::parsing::PathAnalysis;

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
    fn test_get_path_target() {
        // preparation
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        cg_data.add_lib_src_files().unwrap();
        cg_data.expand_use_groups().unwrap();

        // get use entries from cg_fusion_lib_test
        let (cg_fusion_lib_test_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "cg_fusion_lib_test")
            .unwrap();
        let use_statements_of_cg_fusion_lib_test: Vec<(NodeIndex, Ident, &UseTree)> = cg_data
            .iter_syn_neighbors(cg_fusion_lib_test_index)
            .filter_map(|(n, i)| {
                if let Item::Use(item_use) = i {
                    ItemName::from(i)
                        .extract_ident()
                        .map(|id| (n, id, &item_use.tree))
                } else {
                    None
                }
            })
            .collect();
        // get use entries, which point to lib crates
        let (use_index_my_map_two_dim, _, use_tree_my_map_two_dim) =
            use_statements_of_cg_fusion_lib_test
                .iter()
                .find(|(_, id, _)| id == "my_map_two_dim")
                .unwrap();
        let (use_index_my_array, _, use_tree_my_array) = use_statements_of_cg_fusion_lib_test
            .iter()
            .find(|(_, id, _)| id == "my_array")
            .unwrap();
        let (my_map_two_dim_mod_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "my_map_two_dim")
            .unwrap();
        let (my_array_mod_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "my_array")
            .unwrap();

        // test path target of use items, which point to lib crates
        assert_eq!(
            cg_data
                .get_path_target(*use_index_my_map_two_dim, *use_tree_my_map_two_dim)
                .unwrap(),
            PathTarget::Item(my_map_two_dim_mod_index)
        );
        assert_eq!(
            cg_data
                .get_path_target(*use_index_my_array, *use_tree_my_array)
                .unwrap(),
            PathTarget::Item(my_array_mod_index)
        );

        // get use entries, which point to items in modules
        let (use_index_my_array_struct, _, use_tree_my_array_struct) =
            use_statements_of_cg_fusion_lib_test
                .iter()
                .find(|(_, id, _)| id == "MyArray")
                .unwrap();
        let (use_index_my_map_2d_struct, _, use_tree_my_map_2d_struct) =
            use_statements_of_cg_fusion_lib_test
                .iter()
                .find(|(_, id, _)| id == "MyMap2D")
                .unwrap();
        let (use_index_my_map_point_struct, _, use_tree_my_map_point_struct) =
            use_statements_of_cg_fusion_lib_test
                .iter()
                .find(|(_, id, _)| id == "MapPoint")
                .unwrap();
        // get item indices
        let (my_map_point_mod_index, _) = cg_data
            .iter_syn_items(my_map_two_dim_mod_index)
            .filter_map(|(n, i)| {
                if let Item::Mod(_) = i {
                    ItemName::from(i).extract_ident().map(|id| (n, id))
                } else {
                    None
                }
            })
            .find(|(_, c)| c == "my_map_point")
            .unwrap();
        let my_array_item_index = cg_data
            .iter_syn_neighbors(my_array_mod_index)
            .filter_map(|(n, i)| ItemName::from(i).extract_ident().map(|id| (n, id)))
            .find(|(_, id)| id == "MyArray")
            .unwrap()
            .0;
        let my_map_2d_item_index = cg_data
            .iter_syn_neighbors(my_map_two_dim_mod_index)
            .filter_map(|(n, i)| ItemName::from(i).extract_ident().map(|id| (n, id)))
            .find(|(_, id)| id == "MyMap2D")
            .unwrap()
            .0;
        let my_map_point_item_index = cg_data
            .iter_syn_neighbors(my_map_point_mod_index)
            .filter_map(|(n, i)| ItemName::from(i).extract_ident().map(|id| (n, id)))
            .find(|(_, id)| id == "MapPoint")
            .unwrap()
            .0;
        // test path target of use items, which point to items in modules
        assert_eq!(
            cg_data
                .get_path_target(*use_index_my_array_struct, *use_tree_my_array_struct)
                .unwrap(),
            PathTarget::Item(my_array_item_index)
        );
        assert_eq!(
            cg_data
                .get_path_target(*use_index_my_map_2d_struct, *use_tree_my_map_2d_struct)
                .unwrap(),
            PathTarget::Item(my_map_2d_item_index)
        );
        assert_eq!(
            cg_data
                .get_path_target(
                    *use_index_my_map_point_struct,
                    *use_tree_my_map_point_struct
                )
                .unwrap(),
            PathTarget::Item(my_map_point_item_index)
        );

        // get use entries, which point to use globs
        let use_globs: Vec<(NodeIndex, Ident, &UseTree)> = cg_data
            .iter_syn_neighbors(my_map_two_dim_mod_index)
            .filter_map(|(n, i)| {
                if let Item::Use(item_use) = i {
                    let path = item_use.tree.extract_path().unwrap();
                    ItemName::from(i).is_glob().then_some((
                        n,
                        path.segments.last().unwrap().to_owned(),
                        &item_use.tree,
                    ))
                } else {
                    None
                }
            })
            .collect();

        let (use_glob_index_my_map_point, _, use_glob_tree_my_map_point) = use_globs
            .iter()
            .find(|(_, id, _)| id == "my_map_point")
            .unwrap();
        let (use_glob_index_my_array, _, use_glob_tree_my_array) = use_globs
            .iter()
            .find(|(_, id, _)| id == "my_array")
            .unwrap();
        let (use_glob_index_my_compass, _, use_glob_tree_my_compass) = use_globs
            .iter()
            .find(|(_, id, _)| id == "my_compass")
            .unwrap();

        let my_compass_mod_index = cg_data
            .iter_syn_neighbors(my_map_point_mod_index)
            .filter_map(|(n, i)| {
                if let Item::Mod(_) = i {
                    ItemName::from(i).extract_ident().map(|id| (n, id))
                } else {
                    None
                }
            })
            .find(|(_, id)| id == "my_compass")
            .unwrap()
            .0;

        // test path target of use items, which point to use globs
        assert_eq!(
            cg_data
                .get_path_target(*use_glob_index_my_map_point, *use_glob_tree_my_map_point)
                .unwrap(),
            PathTarget::Glob(my_map_point_mod_index)
        );
        assert_eq!(
            cg_data
                .get_path_target(*use_glob_index_my_array, *use_glob_tree_my_array)
                .unwrap(),
            PathTarget::Glob(my_array_mod_index)
        );
        assert_eq!(
            cg_data
                .get_path_target(*use_glob_index_my_compass, *use_glob_tree_my_compass)
                .unwrap(),
            PathTarget::Glob(my_compass_mod_index)
        );

        // get use entries of my_map_point
        let use_of_my_map_point: Vec<(NodeIndex, Ident, &UseTree)> = cg_data
            .iter_syn_neighbors(my_map_point_mod_index)
            .filter_map(|(n, i)| {
                if let Item::Use(item_use) = i {
                    let path = item_use.tree.extract_path().unwrap();
                    Some((n, path.segments.last().unwrap().to_owned(), &item_use.tree))
                } else {
                    None
                }
            })
            .collect();
        let (use_extern_ordering, _, use_tree_extern_ordering) = use_of_my_map_point
            .iter()
            .find(|(_, id, _)| id == "Ordering")
            .unwrap();
        let (use_glob_index_my_compass, _, use_glob_tree_my_compass) = use_of_my_map_point
            .iter()
            .find(|(_, id, _)| id == "my_compass")
            .unwrap();
        assert_eq!(
            cg_data
                .get_path_target(*use_extern_ordering, *use_tree_extern_ordering)
                .unwrap(),
            PathTarget::ExternalPackage
        );
        assert_eq!(
            cg_data
                .get_path_target(*use_glob_index_my_compass, *use_glob_tree_my_compass)
                .unwrap(),
            PathTarget::Glob(my_compass_mod_index)
        );
    }

    #[test]
    fn test_expand_use_globs_and_link_use_items() {
        // preparation
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        cg_data.add_lib_src_files().unwrap();
        cg_data.expand_use_groups().unwrap();

        // action to test
        cg_data.expand_use_globs_and_link_use_items().unwrap();

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

        let (my_map_two_dim_mod_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "my_map_two_dim")
            .unwrap();
        // using iter_syn_items() here to also collect use statements of my_map_point, which is a module of my_map_two_dim
        let use_statements: Vec<String> = cg_data
            .iter_syn_items(my_map_two_dim_mod_index)
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

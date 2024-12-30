// functions to analyze use statements in src files

use super::{AnalyzeError, AnalyzeState};
use crate::{
    add_context, challenge_tree::PathTarget, configuration::CliInput, error::CgResult,
    parsing::ItemExtras, CgData,
};
use anyhow::{anyhow, Context};
use petgraph::graph::NodeIndex;
use quote::ToTokens;
use std::collections::{HashMap, VecDeque};
use syn::{Item, UseTree, Visibility};

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn expand_and_link_use_statements(&mut self) -> CgResult<()> {
        // ToDo: move max_attempts to options
        let max_attempts: u8 = 5;
        let mut use_indices_and_trees: VecDeque<(NodeIndex, Item, UseTree)> = self
            .iter_crates()
            .flat_map(|(crate_index, ..)| {
                self.iter_syn_items(crate_index).filter_map(|(n, i)| {
                    if let Item::Use(item_use) = i {
                        Some((n, i.to_owned(), item_use.tree.to_owned()))
                    } else {
                        None
                    }
                })
            })
            .collect();
        let mut use_attempts: HashMap<NodeIndex, u8> = HashMap::new();
        // expand use statements and link to target
        while let Some((use_index, item, use_tree)) = use_indices_and_trees.pop_front() {
            if item.contains_use_group() {
                // expand use group
                for (new_use_item_index, new_use_item, new_use_tree) in
                    self.expand_use_group(use_index)?.into_iter()
                {
                    use_indices_and_trees.push_back((
                        new_use_item_index,
                        new_use_item,
                        new_use_tree,
                    ));
                }
                continue;
            }
            if let Some(glob_use_tree) = item.is_use_glob() {
                // expand use glob
                use_indices_and_trees.push_back((use_index, item, use_tree));
                continue;
            }
        }

        self.expand_use_globs_and_link_use_items()?;
        Ok(())
    }
    fn expand_use_group(
        &mut self,
        syn_use_group_index: NodeIndex,
    ) -> CgResult<Vec<(NodeIndex, Item, UseTree)>> {
        // get index of module of syn use item
        let module_index = self
            .get_syn_item_module_index(syn_use_group_index)
            .context(add_context!("Expected source index of syn item."))?;
        // remove old use item from tree
        let old_use_item = self
            .tree
            .remove_node(syn_use_group_index)
            .context(add_context!("Expected syn node to remove"))?
            .get_item_from_syn_item_node()
            .context(add_context!("Expected syn Item."))?
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
        let mut new_use_items: Vec<(NodeIndex, Item, UseTree)> = Vec::new();
        for new_use_item in old_use_item.get_use_items_of_use_group() {
            let new_index = self.add_syn_item(&new_use_item, &"".into(), module_index)?;
            if let Item::Use(item_use) = &new_use_item {
                new_use_items.push((new_index, new_use_item.clone(), item_use.tree.to_owned()));
            }
        }
        Ok(new_use_items)
    }

    pub fn expand_use_groups(&mut self) -> CgResult<()> {
        let syn_use_group_indices: Vec<NodeIndex> = self
            .iter_crates()
            .flat_map(|(n, _, _)| {
                self.iter_syn_items(n)
                    .filter_map(|(n, i)| i.contains_use_group().then_some(n))
            })
            .collect();
        for syn_use_group_index in syn_use_group_indices {
            // get index of module of syn use item
            let module_index = self
                .get_syn_item_module_index(syn_use_group_index)
                .context(add_context!("Expected source index of syn item."))?;
            // remove old use item from tree
            let old_use_item = self
                .tree
                .remove_node(syn_use_group_index)
                .context(add_context!("Expected syn node to remove"))?
                .get_item_from_syn_item_node()
                .context(add_context!("Expected syn Item."))?
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
            for new_use_item in old_use_item.get_use_items_of_use_group() {
                self.add_syn_item(&new_use_item, &"".into(), module_index)?;
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
                self.iter_syn_items(crate_index)
                    .filter_map(|(n, i)| i.is_use_glob().map(|t| (n, t.to_owned())))
            })
            .collect();
        let mut use_glob_attempts: HashMap<NodeIndex, u8> = HashMap::new();
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
                    PathTarget::Group => todo!("Rewrite to analyze groups."),
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
                    self.is_visible_for_module(n, use_glob_owning_module_index)
                        .is_ok_and(|vis| vis)
                        && i.is_use_glob().is_some()
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
                .get_item_from_syn_item_node()
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
            let new_use_items: Vec<Item> = self
                .iter_syn_neighbors(use_glob_target_module_index)
                .filter(|(n, _)| {
                    self.is_visible_for_module(*n, use_glob_owning_module_index)
                        .is_ok_and(|vis| vis)
                })
                .filter(|(n, _)| {
                    !self
                        .iter_syn_neighbors(use_glob_owning_module_index)
                        .any(|(m, _)| *n == m)
                })
                .filter_map(|(_, item)| match item {
                    Item::Use(use_tree) => old_use_item
                        .clone()
                        .replace_glob_with_name_or_rename_use_tree(use_tree.tree.to_owned()),
                    _ => None,
                })
                .collect();
            // add new use items to tree
            for new_use_item in new_use_items {
                let new_use_item_index =
                    self.add_syn_item(&new_use_item, &"".into(), use_glob_target_module_index)?;
                if let Item::Use(item_use) = new_use_item {
                    use_glob_indices.push_back((new_use_item_index, item_use.tree.to_owned()));
                }
            }
        }
        Ok(())
    }

    fn is_visible_for_module(
        &self,
        item_index: NodeIndex,
        module_index: NodeIndex,
    ) -> CgResult<bool> {
        /*
        https://doc.rust-lang.org/reference/visibility-and-privacy.html
        With the notion of an item being either public or private, Rust allows item accesses in two cases:
        1. If an item is public, then it can be accessed externally from some module m if you can access all
        the item’s ancestor modules from m. You can also potentially be able to name the item through re-exports.
        2. If an item is private, it may be accessed by the current module and its descendants.

        We do not check if access from m to all ancestor modules is granted, because we use this function for use glob
        statements, which are checked with "cargo check" and "cargo clippy" at start of program. Therefore only legitimate
        path of use glob statements are possible. We only want to check, which items are visible for use glob expansion.
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
        let item = self
            .get_syn_item(item_index)
            .context(add_context!("Expected syn item."))?;
        // item is not a descendant, therefore we have to analyze visibility
        if let Some(visibility) = item.extract_visibility() {
            match visibility {
                Visibility::Inherited => return Ok(false),
                Visibility::Public(_) => return Ok(true),
                Visibility::Restricted(vis_restricted) => {
                    match self.get_path_target(item_index, vis_restricted.path.as_ref())? {
                        PathTarget::ExternalPackage => return Ok(false), // only local syn items have NodeIndex to link to
                        PathTarget::Group => unreachable!("No group in visibility path."),
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
mod tests;

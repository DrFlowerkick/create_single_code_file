// functions to analyze use statements in src files

use super::{AnalyzeError, AnalyzeState};
use crate::{
    add_context,
    challenge_tree::PathTarget,
    configuration::CliInput,
    error::CgResult,
    parsing::{ItemExtras, ItemName},
    CgData,
};
use anyhow::{anyhow, Context};
use petgraph::stable_graph::NodeIndex;
use quote::ToTokens;
use std::collections::{HashMap, VecDeque};
use syn::{Ident, Item, Visibility};

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn expand_and_link_use_statements(&mut self) -> CgResult<()> {
        let mut use_indices_and_path_targets: VecDeque<(NodeIndex, PathTarget)> = self
            .iter_crates()
            .flat_map(|(crate_index, ..)| {
                self.iter_syn_items(crate_index).filter_map(|(n, i)| {
                    if let Item::Use(item_use) = i {
                        self.get_path_target(n, &item_use.tree)
                            .ok()
                            .map(|pt| (n, pt))
                    } else {
                        None
                    }
                })
            })
            .collect();
        // ToDo: move max_attempts to options
        let max_attempts: u8 = 5;
        let mut use_attempts: HashMap<NodeIndex, u8> = HashMap::new();
        // expand use statements and link to target
        while let Some((use_index, use_path_target)) = use_indices_and_path_targets.pop_front() {
            match use_path_target {
                PathTarget::Group => {
                    // expand use group
                    for (new_use_item_index, new_source_path) in
                        self.expand_use_group(use_index)?.into_iter()
                    {
                        use_indices_and_path_targets
                            .push_back((new_use_item_index, new_source_path));
                    }
                    continue;
                }
                PathTarget::Glob(use_glob_target_module_index) => {
                    if let Some(new_use_items) =
                        self.expand_use_glob(use_index, use_glob_target_module_index)?
                    {
                        for (new_use_item_index, new_source_path) in new_use_items.into_iter() {
                            use_indices_and_path_targets
                                .push_back((new_use_item_index, new_source_path));
                        }
                        continue;
                    }
                }
                PathTarget::ExternalPackage => continue, // external package, no need to expand or link
                PathTarget::Item(target_index) | PathTarget::ItemRenamed(target_index, _) => {
                    // Link use item
                    self.add_usage_link(use_index, target_index)?;
                    continue;
                }
                PathTarget::PathCouldNotBeParsed => (), // path could not be parsed, probably because of use glob in path
            }
            // use statement could not be expanded or linked to target, try again after expanding other use statements
            // reasons for not expanding or linking are:
            // - use group, which will be expanded later
            // - use glob, which does not point to the owning module of the use glob
            if *use_attempts
                .entry(use_index)
                .and_modify(|attempts| *attempts += 1)
                .or_insert(1)
                >= max_attempts
            {
                // too many attempts to expand use statement
                // get index and name of module, which owns the use statement
                let use_statement_owning_module_index =
                    self.get_syn_item_module_index(use_index)
                        .context(add_context!("Expected index of owning module of use glob."))?;
                let module = self
                    .get_name_of_crate_or_module(use_statement_owning_module_index)
                    .context(add_context!("Expected crate or module name."))?;
                Err(AnalyzeError::MaxAttemptsExpandingUseStatement(
                    self.get_syn_use_tree(use_index)
                        .context(add_context!("Expected syn use tree."))?
                        .to_token_stream()
                        .to_string(),
                    module,
                ))?;
            }
            use_indices_and_path_targets.push_back((use_index, use_path_target));
        }
        Ok(())
    }

    fn expand_use_group(
        &mut self,
        syn_use_group_index: NodeIndex,
    ) -> CgResult<Vec<(NodeIndex, PathTarget)>> {
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
                old_use_item.get_item_use().unwrap().to_token_stream()
            );
        }
        // expand and collect use items and add them to tree
        let mut new_use_items: Vec<(NodeIndex, PathTarget)> = Vec::new();
        for new_use_item in old_use_item.get_use_items_of_use_group() {
            let new_index = self.add_syn_item(&new_use_item, &"".into(), module_index)?;
            if let Item::Use(item_use) = &new_use_item {
                let new_path_target = self.get_path_target(new_index, &item_use.tree)?;
                new_use_items.push((new_index, new_path_target));
            }
        }
        Ok(new_use_items)
    }

    fn expand_use_glob(
        &mut self,
        use_glob_index: NodeIndex,
        use_glob_target_module_index: NodeIndex,
    ) -> CgResult<Option<Vec<(NodeIndex, PathTarget)>>> {
        // get index and name of module, which owns the use statement
        let use_statement_owning_module_index = self
            .get_syn_item_module_index(use_glob_index)
            .context(add_context!("Expected index of owning module of use glob."))?;

        // collect visible items of target module
        let mut visible_items: Vec<Ident> = Vec::new();
        for (n, i) in self
            .iter_syn_neighbors(use_glob_target_module_index)
            .filter(|(n, _)| {
                self.is_visible_for_module(*n, use_statement_owning_module_index)
                    .is_ok_and(|vis| vis)
            })
        {
            // catch all use statements, which we will not expand or prevent expansion
            if let Item::Use(item_use) = i {
                match self.get_path_target(n, &item_use.tree)? {
                    PathTarget::Group => return Ok(None), // first expand all use groups
                    PathTarget::Glob(glob_target_index) => {
                        // check if glob target module is equal to owning module of use glob
                        if glob_target_index == use_statement_owning_module_index {
                            // ignore use glob, which points to the owning module of the use glob
                            continue;
                        }
                        // first expand all use globs, which do not point to the owning module of the use glob
                        return Ok(None);
                    }
                    PathTarget::PathCouldNotBeParsed => return Ok(None), // If path could not be parsed, it probably contains a use glob
                    PathTarget::ExternalPackage => (),
                    PathTarget::Item(item_index)
                    | PathTarget::ItemRenamed(item_index, _) => {
                        let use_item_owning_module_index = self
                            .get_syn_item_module_index(item_index)
                            .context(add_context!("Expected index of owning module of use item."))?;
                        if use_item_owning_module_index == use_statement_owning_module_index {
                            // ignore use item, which points to item inside the owning module of the use glob
                            continue;
                        }
                    },
                }
            }
            let ident: Ident = match ItemName::from(i) {
                ItemName::Glob | ItemName::Group => {
                    unreachable!("Glob and Group has been evaluated with PathTarget")
                }
                ItemName::TypeStringAndIdent(_, id) => id,
                ItemName::TypeStringAndRenamed(_, _, rename) => rename,
                ItemName::None
                | ItemName::TypeString(_)
                | ItemName::TypeStringAndNameString(_, _) => continue, // No ident, no use import
            };
            visible_items.push(ident);
        }
        // remove old use glob item from tree
        let old_use_item = self
            .tree
            .remove_node(use_glob_index)
            .context(add_context!("Expected syn node to remove"))?
            .get_item_from_syn_item_node()
            .context(add_context!("Expected syn ItemUse."))?
            .to_owned();
        if self.options.verbose() {
            // get name of module, which owns the use glob
            let use_statement_owning_module_name = self
                .get_name_of_crate_or_module(use_statement_owning_module_index)
                .context(add_context!("Expected crate or module name."))?;
            if visible_items.is_empty() {
                println!(
                    "No visible items for use glob statement of module {}:\n{}",
                    use_statement_owning_module_name,
                    old_use_item.get_item_use().unwrap().to_token_stream()
                );
            } else {
                println!(
                    "Expanding use glob statement of module {}:\n{}",
                    use_statement_owning_module_name,
                    old_use_item.get_item_use().unwrap().to_token_stream()
                );
            }
        }
        // expand and collect use items of use glob and add them to tree
        let mut new_use_items: Vec<(NodeIndex, PathTarget)> = Vec::new();
        for new_use_ident in visible_items {
            let new_use_item = old_use_item
                .clone()
                .replace_glob_with_name_ident(new_use_ident)
                .context(add_context!("Expected syn use glob to be replaced."))?;
            let new_index =
                self.add_syn_item(&new_use_item, &"".into(), use_statement_owning_module_index)?;
            if let Item::Use(item_use) = &new_use_item {
                let new_path_target = self.get_path_target(new_index, &item_use.tree)?;
                new_use_items.push((new_index, new_path_target));
            }
        }
        Ok(Some(new_use_items))
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

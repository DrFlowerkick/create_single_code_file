// functions to analyze use statements in src files

use super::{ProcessingCrateUseAndPathState, ProcessingError, ProcessingResult};
use crate::{
    CgData, add_context,
    challenge_tree::PathElement,
    configuration::CgCli,
    parsing::{ItemExt, ItemName, SourcePath, ToTokensExt},
};
use anyhow::{Context, anyhow};
use petgraph::stable_graph::NodeIndex;
use std::collections::{HashMap, VecDeque};
use syn::{Ident, Item, Visibility};

pub struct ProcessingUsageState;

impl<O: CgCli> CgData<O, ProcessingUsageState> {
    pub fn expand_use_statements(
        mut self,
    ) -> ProcessingResult<CgData<O, ProcessingCrateUseAndPathState>> {
        let mut use_groups_and_globs: VecDeque<(NodeIndex, ItemName)> = self
            .iter_crates()
            .flat_map(|(crate_index, ..)| {
                self.iter_syn_items(crate_index)
                    .map(|(n, i)| (n, ItemName::from(i)))
                    .filter(|(_, i)| matches!(i, ItemName::Glob | ItemName::Group))
            })
            .collect();
        let mut use_attempts: HashMap<NodeIndex, u8> = HashMap::new();
        // expand use statements and link to target
        while let Some((use_index, use_item_name)) = use_groups_and_globs.pop_front() {
            match use_item_name {
                ItemName::Group => {
                    // expand use group
                    for new_use_glob in self.expand_use_group(use_index)?.into_iter() {
                        // add any new use glob to queue for expansion
                        use_groups_and_globs.push_back((new_use_glob, ItemName::Glob));
                    }
                }
                ItemName::Glob => {
                    // expand use glob
                    if self.expand_use_glob(use_index)? {
                        // expansion was blocked, try again
                        // reasons for not expanding are:
                        // - visible use group, which will be expanded later
                        // - visible use glob, which does not point to the owning module of the current use glob
                        // - use glob path could not be parsed, probably because of module in path hidden behind some not expanded use glob
                        if *use_attempts
                            .entry(use_index)
                            .and_modify(|attempts| *attempts += 1)
                            .or_insert(1)
                            >= self.options.processing().glob_expansion_max_attempts
                        {
                            // too many attempts to expand use statement
                            // get index and name of module, which owns the use statement
                            let use_statement_owning_module_index =
                                self.get_syn_module_index(use_index).context(add_context!(
                                    "Expected index of owning module of use glob."
                                ))?;
                            let module = self
                                .get_name_of_crate_or_module(use_statement_owning_module_index)
                                .context(add_context!("Expected crate or module name."))?;
                            Err(ProcessingError::MaxAttemptsExpandingUseStatement(
                                self.get_syn_item(use_index)
                                    .context(add_context!("Expected syn item."))?
                                    .get_item_use()
                                    .context(add_context!("Expected use item"))?
                                    .to_trimmed_token_string(),
                                module,
                            ))?;
                        }
                        use_groups_and_globs.push_back((use_index, ItemName::Glob));
                    }
                }
                _ => unreachable!("Filtering for groups and globs"),
            }
        }
        // check, if any use statement remains, which could not be parsed
        if self
            .iter_crates()
            .flat_map(|(n, ..)| self.iter_syn_items(n))
            .any(|(n, i)| {
                if let Item::Use(item_use) = i {
                    match self.get_path_leaf(n, SourcePath::from(item_use)) {
                        Ok(PathElement::PathCouldNotBeParsed) | Err(_) => true,
                        _ => false,
                    }
                } else {
                    false
                }
            })
        {
            return Err(ProcessingError::UseStatementsCouldNotBeParsed);
        }
        Ok(self.set_state(ProcessingCrateUseAndPathState))
    }

    fn expand_use_group(&mut self, use_group_index: NodeIndex) -> ProcessingResult<Vec<NodeIndex>> {
        // get index of module of syn use item
        let module_index = self
            .get_syn_module_index(use_group_index)
            .context(add_context!("Expected source index of syn item."))?;
        // remove old use item from tree
        let old_use_item = self
            .tree
            .remove_node(use_group_index)
            .context(add_context!("Expected syn node to remove"))?
            .get_item_from_syn_item_node()
            .context(add_context!("Expected syn Item."))?
            .to_owned();
        if self.options.verbose() {
            let module = self
                .get_verbose_name_of_tree_node(module_index)
                .context(add_context!("Expected crate or module name."))?;
            println!(
                "Expanding use group statement of {}:\n{}",
                module,
                old_use_item
                    .get_item_use()
                    .unwrap()
                    .to_trimmed_token_string()
            );
        }
        // expand and collect use globs and add them to tree
        let mut use_globs: Vec<NodeIndex> = Vec::new();
        let mut new_use_indices: Vec<NodeIndex> = Vec::new();
        for new_use_item in old_use_item.get_use_items_of_use_group() {
            let new_use_index = self.add_syn_item(&new_use_item, &"".into(), module_index)?;
            new_use_indices.push(new_use_index);
            if let ItemName::Glob = ItemName::from(&new_use_item) {
                use_globs.push(new_use_index);
            }
        }
        self.insert_new_use_indices_in_item_order(module_index, use_group_index, new_use_indices)?;
        Ok(use_globs)
    }

    fn expand_use_glob(&mut self, use_glob_index: NodeIndex) -> ProcessingResult<bool> {
        // get index and name of module, which owns the use statement
        let use_statement_owning_module_index = self
            .get_syn_module_index(use_glob_index)
            .context(add_context!("Expected index of owning module of use glob."))?;
        // get index of module use glob is pointing to
        let use_glob_target_module_index = match self.get_use_item_leaf(use_glob_index)? {
            PathElement::Glob(glob_leaf_index) => glob_leaf_index,
            PathElement::ExternalItem(_) | PathElement::ExternalGlob(_) => return Ok(false), // ignoring external globs and items
            // path of use glob could not be parsed, probably because of module in path, which is "hidden" behind a use glob
            PathElement::PathCouldNotBeParsed => return Ok(true),
            PathElement::Group => {
                return Err(anyhow!(add_context!("Expected expanded groups.")).into());
            }
            PathElement::Item(_) | PathElement::ItemRenamed(_, _) => {
                return Err(anyhow!(add_context!("Expected Glob path leaf")).into());
            }
        };
        // collect visible items of target module
        let mut visible_items: Vec<Ident> = Vec::new();
        for (index_of_item_to_import, item_to_import) in self
            .iter_syn_item_neighbors(use_glob_target_module_index)
            .filter(|(n, _)| {
                self.is_visible_for_module(*n, use_statement_owning_module_index)
                    .is_ok_and(|vis| vis)
            })
        {
            if let Item::Use(item_use) = item_to_import {
                let item_source_path = SourcePath::from(item_use);
                match self.get_path_leaf(index_of_item_to_import, item_source_path.clone()) {
                    Ok(ref path_element) => {
                        match path_element {
                            // first expand all use groups
                            PathElement::Group => return Ok(true),
                            PathElement::Glob(glob_target_index) => {
                                // check if glob target module is equal to owning module of use glob
                                // if yes, ignore use glob, which points to the owning module of the use glob
                                if *glob_target_index != use_statement_owning_module_index {
                                    // first expand all use globs, which do not point to the owning module of the use glob
                                    return Ok(true);
                                }
                            }
                            // If path could not be parsed, it probably contains a module 'hidden' behind use glob
                            PathElement::PathCouldNotBeParsed => return Ok(true),
                            PathElement::ExternalItem(external_item) => {
                                if !self
                                    .iter_syn_item_neighbors(use_statement_owning_module_index)
                                    .filter_map(|(_, i)| {
                                        ItemName::from(i).get_ident_in_name_space()
                                    })
                                    .any(|i| i == *external_item)
                                {
                                    // add ident of external dependency to list of visible items, if it does not
                                    // exist in the owning module of the use glob
                                    visible_items.push(external_item.to_owned());
                                }
                            }
                            PathElement::ExternalGlob(_) => {
                                // do not import external use glob and provide warning to user about it
                                println!(
                                    "\
                            WARNING: use glob '{}' in module '{}' is importing\n\
                            external use glob '{}' of from module '{}'.\n\
                            cg-fusion does mot support this and will ignore \
                            external use globs during expansion of use globs.\n\
                            This may result in unwanted behavior. It can be circumvented \
                            by avoiding use globs of external dependencies.",
                                    self.get_syn_item(use_glob_index)
                                        .unwrap()
                                        .get_item_use()
                                        .unwrap()
                                        .to_trimmed_token_string(),
                                    self.get_verbose_name_of_tree_node(
                                        use_statement_owning_module_index
                                    )?,
                                    item_use.to_trimmed_token_string(),
                                    self.get_verbose_name_of_tree_node(
                                        use_glob_target_module_index
                                    )?,
                                );
                            }
                            PathElement::Item(item_index)
                            | PathElement::ItemRenamed(item_index, _) => {
                                if !self
                                    .iter_syn_item_neighbors(use_statement_owning_module_index)
                                    .any(|(n, _)| n == *item_index)
                                {
                                    // add target of use item to list of visible items, if use statement does point
                                    // to item inside the owning module of the use glob
                                    if let Some(ident_to_import) =
                                        ItemName::from(item_to_import).get_ident_in_name_space()
                                    {
                                        visible_items.push(ident_to_import);
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        unreachable!("Every use statement must have a leave.")
                    }
                }
            } else {
                // if not use item, add ident_to_import to list of visible items
                if let Some(ident_to_import) =
                    ItemName::from(item_to_import).get_ident_in_name_space()
                {
                    visible_items.push(ident_to_import);
                }
            }
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
                .get_verbose_name_of_tree_node(use_statement_owning_module_index)
                .context(add_context!("Expected crate or module name."))?;
            if visible_items.is_empty() {
                println!(
                    "No visible items for use glob statement of module {}:\n{}",
                    use_statement_owning_module_name,
                    old_use_item
                        .get_item_use()
                        .unwrap()
                        .to_trimmed_token_string()
                );
            } else {
                println!(
                    "Expanding use glob statement of {}:\n{}",
                    use_statement_owning_module_name,
                    old_use_item
                        .get_item_use()
                        .unwrap()
                        .to_trimmed_token_string()
                );
            }
        }
        // expand use items of use glob and add them to tree
        let mut new_use_indices: Vec<NodeIndex> = Vec::new();
        for new_use_ident in visible_items.into_iter() {
            let new_use_item = old_use_item
                .clone()
                .replace_glob_with_name_ident(new_use_ident)
                .context(add_context!("Expected syn use glob to be replaced."))?;
            new_use_indices.push(self.add_syn_item(
                &new_use_item,
                &"".into(),
                use_statement_owning_module_index,
            )?);
        }
        self.insert_new_use_indices_in_item_order(
            use_statement_owning_module_index,
            use_glob_index,
            new_use_indices,
        )?;
        Ok(false)
    }

    fn is_visible_for_module(
        &self,
        item_index: NodeIndex,
        module_index: NodeIndex,
    ) -> ProcessingResult<bool> {
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
                    match self.get_path_leaf(item_index, vis_restricted.into())? {
                        PathElement::ExternalItem(_) | PathElement::ExternalGlob(_) => {
                            return Ok(false);
                        } // only local syn items have NodeIndex to link to
                        PathElement::Group => unreachable!("No group in visibility path."),
                        PathElement::Glob(_) => unreachable!("No glob in visibility path."),
                        PathElement::ItemRenamed(_, _) => {
                            unreachable!("No rename in visibility path.")
                        }
                        PathElement::Item(vis_path_module_index) => {
                            if self.is_item_descendant_of_or_same_module(
                                vis_path_module_index,
                                module_index,
                            ) {
                                return Ok(true);
                            }
                        }
                        PathElement::PathCouldNotBeParsed => return Ok(false),
                    }
                }
            }
        }
        Ok(false)
    }

    fn insert_new_use_indices_in_item_order(
        &mut self,
        module_of_use_item: NodeIndex,
        old_use_index: NodeIndex,
        new_use_indices: Vec<NodeIndex>,
    ) -> ProcessingResult<()> {
        let Some(item_order) = self.item_order.get_mut(&module_of_use_item) else {
            return Err(anyhow!(add_context!("Expected item order of module.")).into());
        };
        let Some(pos_old_use_item) = item_order.iter().position(|o| *o == old_use_index) else {
            return Err(anyhow!(add_context!(
                "Expected position of old use item in item order."
            ))
            .into());
        };
        item_order.splice(pos_old_use_item..=pos_old_use_item, new_use_indices);
        Ok(())
    }
}

#[cfg(test)]
mod tests;

// Linking all items, which are required by challenge

use super::AnalyzeState;
use crate::{
    add_context,
    challenge_tree::{EdgeType, NodeType},
    configuration::CliInput,
    error::CgResult,
    parsing::{IdentVisitor, ItemName},
    CgData,
};
use anyhow::Context;
use petgraph::stable_graph::NodeIndex;
use std::collections::HashSet;
use syn::{visit::Visit, Item};

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn link_required_by_challenge(&mut self) -> CgResult<()> {
        // ToDo: replace semantic wording with required by challenge
        // initialize linking of required items with main function of challenge bin crate
        let (challenge_bin_index, _) = self.get_challenge_bin_crate().unwrap();
        let (main_index, _) = self
            .iter_syn_item_neighbors(challenge_bin_index)
            .filter_map(|(n, i)| match i {
                Item::Fn(fn_item) => {
                    if fn_item.sig.ident == "main" {
                        Some((n, i.to_owned()))
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .next()
            .context(add_context!("Expected main fn of challenge bin crate."))?;
        self.add_required_by_challenge_link(challenge_bin_index, main_index)?;
        // a seen cache to make sure, that every required item is only checked once for path statements
        let mut seen_items_path_check: HashSet<NodeIndex> = HashSet::new();
        // ToDo: modules to check should be an iterator over every crate or module, which has an outgoing or incoming
        // RequiredByChallenge edge
        let mut modules_to_check = HashSet::new();
        modules_to_check.insert(challenge_bin_index);
        // ToDo: rework below to name space conform behavior. Only use statements and paths starting with
        // a module or crate can cross name spaces.
        let mut new_challenge_link = true;
        while new_challenge_link {
            new_challenge_link = false;
            for module_index in modules_to_check.clone().iter() {
                if self.options.verbose() {
                    println!(
                        "Checking module {} for semantic linking.",
                        self.get_verbose_name_of_tree_node(*module_index).unwrap()
                    );
                }
                // ToDo: check if filter for items with ident should be included in iter_items_of_module_to_check_for_challenge()
                let items_to_check: Vec<NodeIndex> = self
                    .iter_items_of_module_to_check_for_challenge(*module_index)
                    .filter_map(|(n, nt)| match nt {
                        NodeType::SynItem(item) => {
                            ItemName::from(item).get_ident_in_name_space().map(|_| n)
                        }
                        NodeType::SynImplItem(impl_item) => ItemName::from(impl_item)
                            .get_ident_in_name_space()
                            .map(|_| n),
                        _ => None,
                    })
                    .collect();
                for item_index in items_to_check {
                    if self.is_syn_item(item_index) {
                        let syn_item = self.get_syn_item(item_index).unwrap().to_owned();
                        // we filter for items with ident
                        let syn_ident =
                            ItemName::from(&syn_item).get_ident_in_name_space().unwrap();
                        let mut semantic_index: Option<NodeIndex> = None;
                        // check if item is (reimported) module
                        if self.is_module_or_reimported_module(item_index) {
                            // ToDo: check if module is used as part of a path
                        } else {
                            let mut visit_ident = IdentVisitor::new(syn_ident);
                            for (si, semantic_node) in
                                self.iter_items_of_module_required_by_challenge(*module_index)
                            {
                                match semantic_node {
                                    NodeType::SynItem(item) => {
                                        visit_ident.visit_item(item);
                                    }
                                    NodeType::SynImplItem(impl_block) => {
                                        visit_ident.visit_impl_item(impl_block);
                                    }
                                    _ => unreachable!("Expected SynItem or SynImplItem."),
                                }
                                if visit_ident.found {
                                    semantic_index = Some(si);
                                    break;
                                }
                            }
                        }
                        if let Some(si) = semantic_index {
                            new_challenge_link = true;
                            self.add_required_by_challenge_link(item_index, si)?;
                            match &syn_item {
                                Item::Mod(_) => {
                                    // ToDo: how do we check if module is used at start of path?
                                    modules_to_check.insert(item_index);
                                }
                                Item::Use(_) => {
                                    if let Some(use_item_index) = self
                                        .get_parent_index_by_edge_type(item_index, EdgeType::Usage)
                                    {
                                        // if use item is a module or crate, add it to modules_to_check
                                        if self.is_crate_or_module(use_item_index) {
                                            // ToDo: how do we check if module is used at start of path?
                                            modules_to_check.insert(use_item_index);
                                        } else {
                                            // get module of use item and add it to modules_to_check
                                            let module_of_use_item =
                                                self.get_syn_module_index(use_item_index).context(
                                                    add_context!("Expected module of use item."),
                                                )?;
                                            self.add_required_by_challenge_link(
                                                use_item_index,
                                                item_index,
                                            )?;
                                            modules_to_check.insert(module_of_use_item);
                                        }
                                    }
                                }
                                _ => {
                                    // ToDo: check for impl blocks
                                    // traits will be added to semantic_items
                                    // impl blocks of enums, structs or unions will be added to impl_blocks_to_check
                                }
                            }
                        }
                    } else {
                        todo!("Handle impl blocks.");
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::super::tests::setup_analyze_test;
    use super::*;
    use crate::challenge_tree::EdgeType;

    #[test]
    fn test_initial_semantic_linking() {
        // preparation
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        cg_data.add_lib_src_files().unwrap();
        cg_data.expand_use_statements().unwrap();
        cg_data.link_impl_blocks_with_corresponding_item().unwrap();

        // action to test
        // initialize semantic linking with main function of challenge bin crate
        let (challenge_bin_index, _) = cg_data.get_challenge_bin_crate().unwrap();
        let (main_index, _) = cg_data
            .iter_syn_item_neighbors(challenge_bin_index)
            .filter_map(|(n, i)| match i {
                Item::Fn(fn_item) => {
                    if fn_item.sig.ident == "main" {
                        Some((n, i.to_owned()))
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .next()
            .context(add_context!("Expected main fn of challenge bin crate."))
            .unwrap();
        dbg!(cg_data.get_syn_item(main_index));
        cg_data
            .add_required_by_challenge_link(challenge_bin_index, main_index)
            .unwrap();
        let semantic_links: Vec<(NodeIndex, String)> = cg_data
            .iter_items_of_module_required_by_challenge(challenge_bin_index)
            .filter_map(|(n, _)| {
                cg_data
                    .get_verbose_name_of_tree_node(n)
                    .ok()
                    .map(|s| (n, s))
            })
            .collect();
        dbg!(&semantic_links);
        let no_semantic_links: Vec<(NodeIndex, String)> = cg_data
            .iter_items_of_module_to_check_for_challenge(challenge_bin_index)
            .filter_map(|(n, _)| {
                cg_data
                    .get_verbose_name_of_tree_node(n)
                    .ok()
                    .map(|s| (n, s))
            })
            .collect();
        dbg!(&no_semantic_links);
    }

    #[test]
    fn test_around_with_semantic_linking() {
        // preparation
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        cg_data.add_lib_src_files().unwrap();
        cg_data.expand_use_statements().unwrap();
        cg_data.link_impl_blocks_with_corresponding_item().unwrap();

        // action to test
        cg_data.link_required_by_challenge().unwrap();

        // assertion
        let items_with_semantic_link: Vec<(NodeIndex, Item)> = cg_data
            .iter_crates()
            .map(|(n, _, _)| cg_data.iter_syn_items(n))
            .flatten()
            .filter(|(n, _)| {
                cg_data
                    .tree
                    .edges(*n)
                    .any(|e| *e.weight() == EdgeType::RequiredByChallenge)
            })
            .map(|(n, i)| (n, i.to_owned()))
            .collect();
        let semantic_items_ident: Vec<String> = items_with_semantic_link
            .iter()
            .map(|(_, i)| format!("{}", ItemName::from(i)))
            .collect();
        dbg!(&semantic_items_ident);
    }
}

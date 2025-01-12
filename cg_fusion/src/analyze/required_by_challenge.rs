// Linking all items, which are required by challenge

use super::AnalyzeState;
use crate::{
    add_context,
    challenge_tree::{EdgeType, NodeType, PathElement, SourcePathWalker},
    configuration::CliInput,
    error::CgResult,
    parsing::{ChallengeCollector, ItemName, PathAnalysis, SourcePath},
    CgData,
};
use anyhow::{anyhow, Context};
use petgraph::stable_graph::NodeIndex;
use std::collections::HashSet;
use syn::{visit::Visit, Item};

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn link_required_by_challenge(&mut self) -> CgResult<()> {
        self.link_required_by_challenge_via_parsing()?;
        self.link_required_by_challenge_via_dialog()?;
        Ok(())
    }

    fn link_required_by_challenge_via_parsing(&mut self) -> CgResult<()> {
        // initialize linking of required items with main function of challenge bin crate
        let (challenge_bin_index, _) = self.get_challenge_bin_crate().unwrap();
        let (main_index, _) = self
            .iter_syn_item_neighbors(challenge_bin_index)
            .find_map(|(n, i)| match i {
                Item::Fn(fn_item) => {
                    if fn_item.sig.ident == "main" {
                        Some((n, i.to_owned()))
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .context(add_context!("Expected main fn of challenge bin crate."))?;
        self.add_required_by_challenge_link(challenge_bin_index, main_index)?;
        // a seen cache to make sure, that every required item is only checked once for path statements
        let mut seen_check_items: HashSet<NodeIndex> = HashSet::new();
        self.check_path_items_for_challenge(main_index, &mut seen_check_items)?;
        // mark all trait items of required trait as required by challenge
        let trait_items: Vec<(NodeIndex, NodeIndex)> = self
            .iter_items_required_by_challenge()
            .filter_map(|(n, nt)| match nt {
                NodeType::SynItem(Item::Trait(_)) => Some(n),
                _ => None,
            })
            .flat_map(|n| self.iter_syn_trait_item(n).map(move |(nti, _)| (nti, n)))
            .filter(|(n, _)| !self.is_required_by_challenge(*n))
            .collect();
        for (trait_item_index, trait_index) in trait_items {
            self.add_required_by_challenge_link(trait_index, trait_item_index)?;
            self.check_path_items_for_challenge(trait_item_index, &mut seen_check_items)?;
        }
        // mark all impl items of required impl with trait as required by challenge
        let impl_with_trait_items: Vec<(NodeIndex, NodeIndex)> = self
            .iter_items_required_by_challenge()
            .filter_map(|(n, nt)| match nt {
                NodeType::SynItem(Item::Impl(item_impl)) => item_impl.trait_.is_some().then_some(n),
                _ => None,
            })
            .flat_map(|n| self.iter_syn_impl_item(n).map(move |(nii, _)| (nii, n)))
            .filter(|(n, _)| !self.is_required_by_challenge(*n))
            .collect();
        for (impl_with_trait_item_index, trait_index) in impl_with_trait_items {
            self.add_required_by_challenge_link(trait_index, impl_with_trait_item_index)?;
            self.check_path_items_for_challenge(impl_with_trait_item_index, &mut seen_check_items)?;
        }
        Ok(())
    }

    fn link_required_by_challenge_via_dialog(&mut self) -> CgResult<()> {
        let mut seen_dialog_items: HashSet<NodeIndex> = HashSet::new();
        let mut seen_check_items: HashSet<NodeIndex> = self
            .iter_items_required_by_challenge()
            .map(|(n, _)| n)
            .collect();
        while let Some(dialog_item) =
            self.find_impl_item_without_required_link_in_required_impl_block(&seen_dialog_items)
        {
            seen_dialog_items.insert(dialog_item);
            let impl_block_index = self
                .get_parent_index_by_edge_type(dialog_item, EdgeType::Syn)
                .unwrap();
            println!(
                "Found '{}' of required '{}'.",
                self.get_verbose_name_of_tree_node(dialog_item)?,
                self.get_verbose_name_of_tree_node(impl_block_index)?
            );
            // ToDo: Dialog setup. We want to test dialogs with mock. Probably will use dialoguer
            // for cmd dialog.
            let user_input: bool = unimplemented!("Create dialog fn");
            if user_input {
                self.add_required_by_challenge_link(impl_block_index, dialog_item)?;
                self.check_path_items_for_challenge(dialog_item, &mut seen_check_items)?;
            }
        }
        Ok(())
    }

    fn check_path_items_for_challenge(
        &mut self,
        item_to_check: NodeIndex,
        seen_check_items: &mut HashSet<NodeIndex>,
    ) -> CgResult<()> {
        if seen_check_items.insert(item_to_check) {
            let mut challenge_collector = ChallengeCollector::new();
            match self.tree.node_weight(item_to_check) {
                Some(NodeType::SynItem(Item::Mod(_)))            // do not search path in these items, since
                | Some(NodeType::SynItem(Item::Impl(_)))         // we will search in their sub items if they
                | Some(NodeType::SynItem(Item::Trait(_))) => (), // are linked as required by challenge.
                Some(NodeType::SynItem(Item::Use(item_use))) => {
                    let source_path = item_use.tree.extract_path();
                    match source_path {
                        SourcePath::Glob(_) | SourcePath::Group => {
                            return Err(anyhow!(format!(
                                "{}",
                                add_context!("Expected expanded use groups and 0globs")
                            ))
                            .into())
                        }
                        SourcePath::Name(_) | SourcePath::Rename(_, _) => {
                            challenge_collector.paths.push(source_path)
                        }
                    }
                }
                Some(NodeType::SynItem(item)) => challenge_collector.visit_item(item),
                Some(NodeType::SynImplItem(impl_item)) => challenge_collector.visit_impl_item(impl_item),
                Some(NodeType::SynTraitItem(trait_item)) => {
                    challenge_collector.visit_trait_item(trait_item)
                }
                _ => return Ok(()),
            }
            // check collected path elements
            for path in challenge_collector.paths.iter() {
                let mut path_walker =
                    SourcePathWalker::new(path.extract_path(), self, item_to_check);
                while let Some(path_element) = path_walker.next(self) {
                    match path_element {
                        PathElement::Glob(_) | PathElement::Group => unreachable!("syn::Path does not contain these elements and all use statements must be expanded."),
                        PathElement::ExternalPackage => (),
                        PathElement::PathCouldNotBeParsed => (), // ToDo: we will try later to analyze path to let statements
                        PathElement::Item(item_index) | PathElement::ItemRenamed(item_index, _) => {
                            if self.is_syn_impl_item(item_index) || self.is_syn_trait_item(item_index) {
                                let impl_or_trait_index = self.get_parent_index_by_edge_type(item_index, EdgeType::Syn)
                                    .context(add_context!("Expected impl or trait item."))?;
                                if !self.is_required_by_challenge(impl_or_trait_index) {
                                    self.add_required_by_challenge_link(item_to_check, impl_or_trait_index)?;
                                }
                            }
                            self.add_required_by_challenge_link(item_to_check, item_index)?;
                            self.check_path_items_for_challenge(item_index, seen_check_items)?;
                        },
                    }
                }
            }
            // check collected method calls with self as receiver
            if self.is_syn_impl_item(item_to_check) {
                for method_id in challenge_collector.self_method_calls.iter() {
                    let impl_method = self
                        .get_parent_index_by_edge_type(item_to_check, EdgeType::Syn)
                        .into_iter()
                        .flat_map(|n| self.iter_syn_impl_item(n))
                        .filter(|(n, _)| !self.is_required_by_challenge(*n))
                        .find(|(_, i)| {
                            if let Some(name) = ItemName::from(*i).get_ident_in_name_space() {
                                name == *method_id
                            } else {
                                false
                            }
                        });
                    if let Some((item_index, _)) = impl_method {
                        self.add_required_by_challenge_link(item_to_check, item_index)?;
                        self.check_path_items_for_challenge(item_index, seen_check_items)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn find_impl_item_without_required_link_in_required_impl_block(
        &self,
        seen_dialog_items: &HashSet<NodeIndex>,
    ) -> Option<NodeIndex> {
        None
    }
}

#[cfg(test)]
mod tests {

    use super::super::tests::setup_analyze_test;
    use super::*;

    #[test]
    fn test_initial_challenge_linking() {
        // preparation
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        cg_data.add_lib_src_files().unwrap();
        cg_data.expand_use_statements().unwrap();
        cg_data.link_impl_blocks_with_corresponding_item().unwrap();

        // action to test
        // initialize challenge linking with main function of challenge bin crate
        let (challenge_bin_index, _) = cg_data.get_challenge_bin_crate().unwrap();
        let (main_index, main_fn) = cg_data
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
        let mut challenge_collector = ChallengeCollector::new();
        challenge_collector.visit_item(&main_fn);
        let path_leafs: Vec<(&SourcePath, PathElement, String)> = challenge_collector
            .paths
            .iter()
            .filter_map(|sp| {
                cg_data
                    .get_path_leaf(main_index, sp)
                    .ok()
                    .map(|pe| (sp, pe))
            })
            .filter_map(|(sp, pe)| match pe {
                PathElement::Item(item_index) | PathElement::ItemRenamed(item_index, _) => Some((
                    sp,
                    pe,
                    cg_data.get_verbose_name_of_tree_node(item_index).unwrap(),
                )),
                _ => Some((sp, pe, "".into())),
            })
            .collect();
        dbg!(&path_leafs);
    }

    #[test]
    fn test_around_with_challenge_linking() {
        // preparation
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        cg_data.add_lib_src_files().unwrap();
        cg_data.expand_use_statements().unwrap();
        cg_data.link_impl_blocks_with_corresponding_item().unwrap();

        // action to test
        cg_data.link_required_by_challenge_via_parsing().unwrap();

        // assertion
        let items_required_by_challenge: Vec<NodeIndex> = cg_data
            .iter_items_required_by_challenge()
            .map(|(n, _)| n)
            .collect();
        let mut challenge_items_ident: Vec<String> = items_required_by_challenge
            .iter()
            .map(|n| {
                if let Some(module_index) = cg_data.get_syn_module_index(*n) {
                    format!(
                        "{}::{}",
                        cg_data.get_verbose_name_of_tree_node(module_index).unwrap(),
                        cg_data.get_verbose_name_of_tree_node(*n).unwrap()
                    )
                } else {
                    format!("{}", cg_data.get_verbose_name_of_tree_node(*n).unwrap())
                }
            })
            .collect();
        challenge_items_ident.sort();
        assert_eq!(
            challenge_items_ident,
            [
                "action (Mod)::Action (Impl)",
                "action (Mod)::Action (Impl)::set_white (Impl Fn)",
                "action (Mod)::Action (Struct)",
                "action (Mod)::MapPoint (Use)",
                "action (Mod)::Value (Use)",
                "action (Mod)::X (Use)",
                "action (Mod)::Y (Use)",
                "cg_fusion_binary_test (binary crate)::Action (Use)",
                "cg_fusion_binary_test (binary crate)::Go (Use)",
                "cg_fusion_binary_test (binary crate)::MapPoint (Use)",
                "cg_fusion_binary_test (binary crate)::X (Use)",
                "cg_fusion_binary_test (binary crate)::Y (Use)",
                "cg_fusion_binary_test (binary crate)::main (Fn)",
                "cg_fusion_binary_test (library crate)",
                "cg_fusion_binary_test (library crate)::Go (Impl)",
                "cg_fusion_binary_test (library crate)::Go (Impl)::new (Impl Fn)",
                "cg_fusion_binary_test (library crate)::Go (Struct)",
                "cg_fusion_binary_test (library crate)::MyMap2D (Use)",
                "cg_fusion_binary_test (library crate)::N (Const)",
                "cg_fusion_binary_test (library crate)::Value (Enum)",
                "cg_fusion_binary_test (library crate)::X (Const)",
                "cg_fusion_binary_test (library crate)::Y (Const)",
                "cg_fusion_binary_test (library crate)::action (Mod)",
                "cg_fusion_lib_test (library crate)",
                "cg_fusion_lib_test (library crate)::my_map_two_dim (Use)",
                "my_map_point (Mod)::MapPoint (Impl)",
                "my_map_point (Mod)::MapPoint (Impl)::new (Impl Fn)",
                "my_map_point (Mod)::MapPoint (Struct)",
                "my_map_two_dim (library crate)",
                "my_map_two_dim (library crate)::Default for MyMap2D (Impl)",
                "my_map_two_dim (library crate)::Default for MyMap2D (Impl)::default (Impl Fn)",
                "my_map_two_dim (library crate)::MyMap2D (Struct)",
                "my_map_two_dim (library crate)::my_map_point (Mod)",
            ]
        );
    }
}

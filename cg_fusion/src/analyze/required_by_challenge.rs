// Linking all items, which are required by challenge

use super::AnalyzeState;
use crate::{
    add_context,
    challenge_tree::{EdgeType, NodeType, PathElement, SourcePathWalker},
    configuration::CliInput,
    error::CgResult,
    parsing::{PathAnalysis, PathCollector, SourcePath},
    CgData,
};
use anyhow::{anyhow, Context};
use petgraph::stable_graph::NodeIndex;
use std::collections::HashSet;
use syn::{visit::Visit, Item};

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn link_required_by_challenge(&mut self) -> CgResult<()> {
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
        let mut seen_items_path_check: HashSet<NodeIndex> = HashSet::new();
        self.check_path_items_for_challenge(main_index, &mut seen_items_path_check)?;
        Ok(())
    }

    fn check_path_items_for_challenge(
        &mut self,
        item_to_check: NodeIndex,
        seen_items_path_check: &mut HashSet<NodeIndex>,
    ) -> CgResult<()> {
        if seen_items_path_check.insert(item_to_check) {
            let mut path_collector = PathCollector::new();
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
                            path_collector.paths.push(source_path)
                        }
                    }
                }
                Some(NodeType::SynItem(item)) => path_collector.visit_item(item),
                Some(NodeType::SynImplItem(impl_item)) => path_collector.visit_impl_item(impl_item),
                Some(NodeType::SynTraitItem(trait_item)) => {
                    path_collector.visit_trait_item(trait_item)
                }
                _ => return Ok(()),
            }
            for path in path_collector.paths.iter() {
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
                            self.check_path_items_for_challenge(item_index, seen_items_path_check)?;
                        },
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
        let mut path_collector = PathCollector::new();
        path_collector.visit_item(&main_fn);
        let path_leafs: Vec<(&SourcePath, PathElement, String)> = path_collector
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
        cg_data.link_required_by_challenge().unwrap();

        // assertion
        let items_with_challenge_link: Vec<NodeIndex> = cg_data
            .tree
            .node_indices()
            .filter(|n| {
                cg_data
                    .tree
                    .edges_directed(*n, petgraph::Direction::Incoming)
                    .any(|e| *e.weight() == EdgeType::RequiredByChallenge)
            })
            .collect();
        let mut challenge_items_ident: Vec<String> = items_with_challenge_link
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

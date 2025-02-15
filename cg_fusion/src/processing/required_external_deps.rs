// functions to check usage of external dependencies.
// most external dependencies will already be pulled in,but some case like
// external use globs and extension methods an macros imported via traits
// will be handled by this module.

use super::{FuseChallengeState, ProcessingResult};
use crate::{
    challenge_tree::{NodeType, PathElement},
    configuration::CgCli,
    parsing::{MacroWriteFinder, SourcePath},
    CgData,
};

use petgraph::stable_graph::NodeIndex;
use syn::{visit::Visit, Item};

pub struct ProcessingRequiredExternals;

impl<O: CgCli> CgData<O, ProcessingRequiredExternals> {
    pub fn process_external_dependencies(
        mut self,
    ) -> ProcessingResult<CgData<O, FuseChallengeState>> {
        let required_crates_and_modules: Vec<NodeIndex> = self
            .iter_crates()
            .filter_map(|(n, ..)| self.is_required_by_challenge(n).then_some(n))
            .chain(self.iter_crates().flat_map(|(n, ..)| {
                self.iter_syn_items(n)
                    .filter_map(|(n, i)| matches!(i, Item::Mod(_)).then_some(n))
                    .filter(|n| self.is_required_by_challenge(*n))
            }))
            .collect();
        for node in required_crates_and_modules {
            // check, if crate or module contains use statements of supported external dependencies,
            // which are not required by challenge
            let external_dep_usage: Vec<NodeIndex> = self
                .iter_syn_item_neighbors(node)
                .filter_map(|(n, i)| match i {
                    Item::Use(item_use) => {
                        if let Ok(PathElement::ExternalPackage) =
                            self.get_path_leaf(n, item_use.into())
                        {
                            if let Some(segments) = SourcePath::from(item_use).get_segments() {
                                if segments.iter().any(|seg| {
                                    self.iter_accepted_dependencies()
                                        .any(|(_, acc_dep)| seg == acc_dep)
                                }) {
                                    Some(n)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .filter(|n| !self.is_required_by_challenge(*n))
                .collect();
            for external_dep_usage_node in external_dep_usage {
                self.add_required_by_challenge_link(node, external_dep_usage_node)?;
            }
            // check if any required item contains a write! macro.
            // If yes, search for use statement which imports Write trait.
            let mut write_finder = MacroWriteFinder::new();
            for (_, node_type) in self
                .iter_syn_of_crate_or_module(node)
                .filter(|(n, _)| self.is_required_by_challenge(*n))
            {
                match node_type {
                    NodeType::SynItem(item) => write_finder.visit_item(item),
                    NodeType::SynImplItem(impl_item) => write_finder.visit_impl_item(impl_item),
                    NodeType::SynTraitItem(trait_item) => write_finder.visit_trait_item(trait_item),
                    _ => (),
                }
            }
            if write_finder.found_write {
                let use_write = self.iter_syn_of_crate_or_module(node).find_map(|(n, nt)| {
                    if let NodeType::SynItem(Item::Use(item_use)) = nt {
                        SourcePath::from(item_use)
                            .get_segments()
                            .and_then(|segments| segments.iter().any(|s| s == "Write").then_some(n))
                    } else {
                        None
                    }
                });
                if let Some(use_node) = use_write {
                    self.add_required_by_challenge_link(node, use_node)?;
                }
            }
        }
        Ok(self.set_state(FuseChallengeState))
    }
}

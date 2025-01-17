// functions to navigate the challenge tree

use super::{
    walkers::{PathElement, SourcePathWalker},
    ChallengeTreeError, CrateFile, EdgeType, LocalPackage, NodeType, TreeResult,
};
use crate::{
    add_context,
    configuration::CgCli,
    parsing::{ItemName, PathAnalysis},
    CgData,
};

use anyhow::{anyhow, Context};
use petgraph::{stable_graph::NodeIndex, visit::EdgeRef, Direction};
use syn::{Item, UseTree};

impl<O, S> CgData<O, S> {
    pub(crate) fn challenge_package(&self) -> &LocalPackage {
        if let NodeType::LocalPackage(ref package) = self.tree.node_weight(0.into()).unwrap() {
            return package;
        }
        unreachable!("Challenge package is created at instantiation of CgDate and should always be at index 0.");
    }

    pub(crate) fn get_local_package(&self, node: NodeIndex) -> TreeResult<&LocalPackage> {
        if let NodeType::LocalPackage(dependency) = self
            .tree
            .node_weight(node)
            .ok_or_else(|| ChallengeTreeError::IndexError(node))?
        {
            Ok(dependency)
        } else {
            Err(ChallengeTreeError::NotLocalPackage(node))
        }
    }

    pub(crate) fn get_binary_crate(&self, node: NodeIndex) -> TreeResult<&CrateFile> {
        if let NodeType::BinCrate(crate_file) = self
            .tree
            .node_weight(node)
            .ok_or_else(|| ChallengeTreeError::IndexError(node))?
        {
            Ok(crate_file)
        } else {
            Err(ChallengeTreeError::NotBinaryCrate(node))
        }
    }

    pub(crate) fn get_library_crate(&self, node: NodeIndex) -> TreeResult<&CrateFile> {
        if let NodeType::LibCrate(crate_file) = self
            .tree
            .node_weight(node)
            .ok_or_else(|| ChallengeTreeError::IndexError(node))?
        {
            Ok(crate_file)
        } else {
            Err(ChallengeTreeError::NotLibraryCrate(node))
        }
    }

    #[cfg(test)] // ToDo: check if we really need this
    pub(crate) fn get_challenge_lib_crate(&self) -> Option<(NodeIndex, &CrateFile)> {
        self.iter_package_crates(0.into())
            .filter_map(|(n, crate_type, cf)| if crate_type { Some((n, cf)) } else { None })
            .next()
    }

    pub(crate) fn get_parent_index_by_edge_type(
        &self,
        node: NodeIndex,
        edge_type: EdgeType,
    ) -> Option<NodeIndex> {
        self.tree
            .edges_directed(node, Direction::Incoming)
            .find(|e| *e.weight() == edge_type)
            .map(|e| e.source())
    }

    pub(crate) fn get_syn_module_index(&self, node: NodeIndex) -> Option<NodeIndex> {
        if let Some(parent_index) = self.get_parent_index_by_edge_type(node, EdgeType::Syn) {
            if self.is_crate_or_module(parent_index) {
                Some(parent_index)
            } else {
                self.get_syn_module_index(parent_index)
            }
        } else {
            None
        }
    }

    pub(crate) fn get_syn_item(&self, node: NodeIndex) -> Option<&Item> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeType::SynItem(item) => Some(item),
            _ => None,
        })
    }

    pub(crate) fn get_syn_use_tree(&self, node: NodeIndex) -> Option<&UseTree> {
        if let Some(Item::Use(item_use)) = self.get_syn_item(node) {
            return Some(&item_use.tree);
        }
        None
    }

    pub(crate) fn get_name_of_crate_or_module(&self, node: NodeIndex) -> Option<String> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeType::SynItem(item) => {
                if let Item::Mod(item_mod) = item {
                    Some(item_mod.ident.to_string())
                } else {
                    None
                }
            }
            NodeType::BinCrate(crate_file) | NodeType::LibCrate(crate_file) => {
                Some(crate_file.name.to_owned())
            }
            _ => None,
        })
    }

    pub(crate) fn get_crate_index(&self, node: NodeIndex) -> Option<NodeIndex> {
        if let Some(node_weight) = self.tree.node_weight(node) {
            match node_weight {
                NodeType::BinCrate(_) | NodeType::LibCrate(_) => return Some(node),
                NodeType::SynItem(_) | NodeType::SynImplItem(_) => {
                    if let Some(parent) = self.get_parent_index_by_edge_type(node, EdgeType::Syn) {
                        return self.get_crate_index(parent);
                    }
                }
                _ => (),
            }
        }
        None
    }

    pub(crate) fn get_verbose_name_of_tree_node(&self, node: NodeIndex) -> TreeResult<String> {
        match self
            .tree
            .node_weight(node)
            .context(add_context!("Expected node weight."))?
        {
            NodeType::LocalPackage(local_package) => {
                Ok(format!("{} (local package)", local_package.name))
            }
            NodeType::ExternalSupportedPackage(supported_package) => {
                Ok(format!("{} (supported package)", supported_package))
            }
            NodeType::ExternalUnsupportedPackage(unsupported_package) => {
                Ok(format!("{} (unsupported package)", unsupported_package))
            }
            NodeType::BinCrate(crate_file) => Ok(format!("{} (binary crate)", crate_file.name)),
            NodeType::LibCrate(crate_file) => Ok(format!("{} (library crate)", crate_file.name)),
            NodeType::SynItem(item) => Ok(format!("{}", ItemName::from(item))),
            NodeType::SynImplItem(impl_item) => {
                let syn_impl_item_index = self
                    .get_parent_index_by_edge_type(node, EdgeType::Syn)
                    .context(add_context!("Expected index of impl item"))?;
                Ok(format!(
                    "{}::{}",
                    self.get_verbose_name_of_tree_node(syn_impl_item_index)?,
                    ItemName::from(impl_item)
                ))
            }
            NodeType::SynTraitItem(trait_item) => {
                let syn_trait_item_index = self
                    .get_parent_index_by_edge_type(node, EdgeType::Syn)
                    .context(add_context!("Expected index of trait item"))?;

                Ok(format!(
                    "{}::{}",
                    self.get_verbose_name_of_tree_node(syn_trait_item_index)?,
                    ItemName::from(trait_item)
                ))
            }
        }
    }

    pub(crate) fn is_crate_or_module(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(
                node_weight,
                NodeType::BinCrate(_) | NodeType::LibCrate(_) | NodeType::SynItem(Item::Mod(_))
            );
        }
        false
    }

    pub(crate) fn is_source_item(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(
                node_weight,
                NodeType::BinCrate(_)
                    | NodeType::LibCrate(_)
                    | NodeType::SynItem(_)
                    | NodeType::SynImplItem(_)
            );
        }
        false
    }

    #[allow(dead_code)] // ToDo: check if we really need this
    pub(crate) fn is_syn_item(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(node_weight, NodeType::SynItem(_));
        }
        false
    }

    pub(crate) fn is_syn_impl_item(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(node_weight, NodeType::SynImplItem(_));
        }
        false
    }

    pub(crate) fn is_syn_trait_item(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(node_weight, NodeType::SynTraitItem(_));
        }
        false
    }

    pub(crate) fn is_item_descendant_of_or_same_module(
        &self,
        item_index: NodeIndex,
        mut module_index: NodeIndex,
    ) -> bool {
        if let Some(item_module_index) = self.get_syn_module_index(item_index) {
            if item_module_index == module_index {
                return true;
            }
            while let Some(mi) = self.get_syn_module_index(module_index) {
                if item_module_index == mi {
                    return true;
                }
                module_index = mi;
            }
        }
        false
    }

    pub(crate) fn is_required_by_challenge(&self, node: NodeIndex) -> bool {
        self.tree
            .edges_directed(node, Direction::Incoming)
            .any(|e| *e.weight() == EdgeType::RequiredByChallenge)
    }

    pub(crate) fn get_path_leaf(
        &self,
        path_item_index: NodeIndex,
        path: &impl PathAnalysis,
    ) -> TreeResult<PathElement> {
        SourcePathWalker::new(path.extract_path(), self, path_item_index)
            .into_iter(self)
            .last()
            .context(add_context!("Expected path target."))
            .map_err(|err| err.into())
    }

    pub(crate) fn get_use_item_leaf(
        &self,
        index_of_use_item: NodeIndex,
    ) -> TreeResult<PathElement> {
        if let Some(Item::Use(item_use)) = self.get_syn_item(index_of_use_item) {
            return self.get_path_leaf(index_of_use_item, &item_use.tree);
        }
        Err(anyhow!(add_context!("Expected syn use item")).into())
    }
}

impl<O: CgCli, S> CgData<O, S> {
    pub(crate) fn get_challenge_bin_name(&self) -> &str {
        if self.options.input().input == "main" {
            // if main, use crate name for bin name
            self.challenge_package().name.as_str()
        } else {
            self.options.input().input.as_str()
        }
    }

    pub(crate) fn get_challenge_bin_crate(&self) -> Option<(NodeIndex, &CrateFile)> {
        let bin_name = self.get_challenge_bin_name();
        self.iter_package_crates(0.into())
            .filter_map(|(n, crate_type, cf)| if !crate_type { Some((n, cf)) } else { None })
            .find(|(_, cf)| cf.name == bin_name)
    }
}

// functions to navigate the challenge tree

use super::{
    walkers::{BfsByEdgeType, BfsModuleNameSpace, PathElement, SourcePathWalker},
    ChallengeTreeError, CrateFile, EdgeType, LocalPackage, NodeType, TreeResult,
};
use crate::{
    add_context,
    configuration::CliInput,
    error::CgResult,
    parsing::{ItemName, PathAnalysis},
    CgData,
};

use anyhow::{anyhow, Context};
use petgraph::{stable_graph::NodeIndex, visit::EdgeRef, Direction};
use syn::{Item, UseTree};

impl<O, S> CgData<O, S> {
    pub fn challenge_package(&self) -> &LocalPackage {
        if let NodeType::LocalPackage(ref package) = self.tree.node_weight(0.into()).unwrap() {
            return package;
        }
        unreachable!("Challenge package is created at instantiation of CgDate and should always be at index 0.");
    }

    pub fn get_local_package(&self, node: NodeIndex) -> TreeResult<&LocalPackage> {
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

    pub fn get_binary_crate(&self, node: NodeIndex) -> TreeResult<&CrateFile> {
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

    pub fn get_library_crate(&self, node: NodeIndex) -> TreeResult<&CrateFile> {
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

    fn iter_packages(&self) -> impl Iterator<Item = (NodeIndex, &NodeType)> {
        BfsByEdgeType::new(&self.tree, 0.into(), EdgeType::Dependency)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .fuse()
    }

    pub fn iter_local_packages(&self) -> impl Iterator<Item = (NodeIndex, &LocalPackage)> {
        self.iter_packages().filter_map(|(n, w)| match w {
            NodeType::LocalPackage(package) => Some((n, package)),
            NodeType::ExternalSupportedPackage(_) | NodeType::ExternalUnsupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    pub fn iter_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &NodeType)> {
        // skip first element, which is root of tree and therefore not a dependency
        self.iter_packages().skip(1)
    }

    pub fn iter_accepted_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.iter_dependencies().filter_map(|(n, w)| match w {
            NodeType::LocalPackage(local_package) => Some((n, local_package.name.as_str())),
            NodeType::ExternalSupportedPackage(name) => Some((n, name.as_str())),
            NodeType::ExternalUnsupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    pub fn iter_unsupported_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.iter_dependencies().filter_map(|(n, w)| match w {
            NodeType::ExternalUnsupportedPackage(name) => Some((n, name.as_str())),
            NodeType::LocalPackage(_) | NodeType::ExternalSupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    pub fn iter_external_dependencies(&self) -> impl Iterator<Item = &str> {
        // include elements of rust libraries in iterator
        self.iter_dependencies()
            .filter_map(|(_, w)| match w {
                NodeType::ExternalSupportedPackage(name)
                | NodeType::ExternalUnsupportedPackage(name) => Some(name.as_str()),
                NodeType::LocalPackage(_) => None,
                _ => unreachable!("Dependency edges only target package nodes."),
            })
            .chain(["std", "core", "std"])
    }

    fn iter_package_crates(
        &self,
        package_index: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, bool, &CrateFile)> {
        BfsByEdgeType::new(&self.tree, package_index, EdgeType::Crate)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| match w {
                NodeType::BinCrate(bin_crate_file) => Some((n, false, bin_crate_file)),
                NodeType::LibCrate(lib_crate_file) => Some((n, true, lib_crate_file)),
                _ => None,
            })
            .fuse()
    }

    pub fn get_challenge_lib_crate(&self) -> Option<(NodeIndex, &CrateFile)> {
        self.iter_package_crates(0.into())
            .filter_map(|(n, crate_type, cf)| if crate_type { Some((n, cf)) } else { None })
            .next()
    }

    pub fn iter_crates(&self) -> impl Iterator<Item = (NodeIndex, bool, &CrateFile)> {
        self.iter_local_packages()
            .flat_map(|(pi, _)| self.iter_package_crates(pi))
    }

    pub fn iter_lib_crates(&self) -> impl Iterator<Item = (NodeIndex, &CrateFile)> {
        self.iter_local_packages().filter_map(|(n, _)| {
            self.iter_package_crates(n)
                .filter_map(|(n, crate_type, cf)| if crate_type { Some((n, cf)) } else { None })
                .next()
        })
    }

    pub fn iter_impl_blocks_of_item(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &Item)> {
        self.tree
            .edges_directed(node, Direction::Outgoing)
            .filter(|e| *e.weight() == EdgeType::Implementation)
            .map(|e| e.target())
            .filter_map(|n| self.get_syn_item(n).map(|i| (n, i)))
    }

    pub fn iter_syn_neighbors(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &NodeType)> {
        self.tree
            .edges_directed(node, Direction::Outgoing)
            .filter(|e| *e.weight() == EdgeType::Syn)
            .map(|e| e.target())
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
    }

    pub fn iter_syn_item_neighbors(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &Item)> {
        self.iter_syn_neighbors(node).filter_map(|(n, w)| match w {
            NodeType::SynItem(item) => Some((n, item)),
            _ => None,
        })
    }

    pub fn iter_syn_items(&self, node: NodeIndex) -> impl Iterator<Item = (NodeIndex, &Item)> {
        BfsByEdgeType::new(&self.tree, node, EdgeType::Syn)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| match w {
                NodeType::SynItem(syn_item) => Some((n, syn_item)),
                _ => None,
            })
            .fuse()
    }

    pub fn get_parent_index_by_edge_type(
        &self,
        node: NodeIndex,
        edge_type: EdgeType,
    ) -> Option<NodeIndex> {
        self.tree
            .edges_directed(node, Direction::Incoming)
            .find(|e| *e.weight() == edge_type)
            .map(|e| e.source())
    }

    pub fn get_syn_module_index(&self, node: NodeIndex) -> Option<NodeIndex> {
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

    pub fn get_syn_item(&self, node: NodeIndex) -> Option<&Item> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeType::SynItem(item) => Some(item),
            _ => None,
        })
    }

    pub fn get_syn_use_tree(&self, node: NodeIndex) -> Option<&UseTree> {
        if let Some(Item::Use(item_use)) = self.get_syn_item(node) {
            return Some(&item_use.tree);
        }
        None
    }

    pub fn get_name_of_crate_or_module(&self, node: NodeIndex) -> Option<String> {
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

    pub fn get_crate_index(&self, node: NodeIndex) -> Option<NodeIndex> {
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

    pub fn get_verbose_name_of_tree_node(&self, node: NodeIndex) -> TreeResult<String> {
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
            NodeType::SynImplItem(impl_item) => Ok(format!("{}", ItemName::from(impl_item))),
            NodeType::SynTraitItem(trait_item) => Ok(format!("{}", ItemName::from(trait_item))),
        }
    }

    pub fn is_crate_or_module(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(
                node_weight,
                NodeType::BinCrate(_) | NodeType::LibCrate(_) | NodeType::SynItem(Item::Mod(_))
            );
        }
        false
    }

    pub fn is_module_or_reimported_module(&self, node: NodeIndex) -> bool {
        if let Some(node_type) = self.tree.node_weight(node) {
            match node_type {
                NodeType::BinCrate(_) | NodeType::LibCrate(_) | NodeType::SynItem(Item::Mod(_)) => {
                    return true;
                }
                NodeType::SynItem(Item::Use(item_use)) => {
                    match self.get_path_leaf(node, &item_use.tree) {
                        Ok(PathElement::Item(use_item_index))
                        | Ok(PathElement::ItemRenamed(use_item_index, _)) => {
                            return self.is_module_or_reimported_module(use_item_index)
                        }
                        _ => return false,
                    }
                }
                _ => return false,
            }
        }
        false
    }

    pub fn is_source_item(&self, node: NodeIndex) -> bool {
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

    pub fn is_syn_item(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(node_weight, NodeType::SynItem(_));
        }
        false
    }

    pub fn is_syn_impl_item(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(node_weight, NodeType::SynImplItem(_));
        }
        false
    }

    pub fn is_syn_trait_item(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(node_weight, NodeType::SynTraitItem(_));
        }
        false
    }

    pub fn is_item_descendant_of_or_same_module(
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

    pub fn is_required_by_challenge(&self, node: NodeIndex) -> bool {
        self.tree
            .edges_directed(node, Direction::Incoming)
            .any(|e| *e.weight() == EdgeType::RequiredByChallenge)
    }

    pub fn iter_items_of_module_to_check_for_challenge(
        &self,
        module: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &NodeType)> {
        BfsModuleNameSpace::new(&self.tree, module)
            .into_iter(&self.tree)
            .filter(|n| {
                !self.is_required_by_challenge(*n)
                    && (self.is_syn_item(*n)
                        || self.is_syn_impl_item(*n)
                        || self.is_syn_trait_item(*n))
            })
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter(|(n, nt)| match nt {
                NodeType::SynImplItem(_) | NodeType::SynTraitItem(_) => {
                    if let Some(parent_index) =
                        self.get_parent_index_by_edge_type(*n, EdgeType::Syn)
                    {
                        // only include impl or trait items, if their corresponding item_impl respectively item_trait
                        // is required by challenge
                        self.is_required_by_challenge(parent_index)
                    } else {
                        unreachable!("Expected parent index of impl or trait item.")
                    }
                }
                _ => true,
            })
            .fuse()
    }

    pub fn iter_items_of_module_required_by_challenge(
        &self,
        module: NodeIndex, // or crate
    ) -> impl Iterator<Item = (NodeIndex, &NodeType)> {
        BfsModuleNameSpace::new(&self.tree, module)
            .into_iter(&self.tree)
            .filter(|n| self.is_required_by_challenge(*n))
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter(|(_, nt)| match nt {
                // Do not include mod, impl or trait items, because they contain further items,
                // which will be added on their own to list
                NodeType::SynItem(Item::Impl(_))
                | NodeType::SynItem(Item::Trait(_))
                | NodeType::SynItem(Item::Mod(_)) => false,
                _ => true,
            })
            .fuse()
    }

    pub fn get_path_root(
        &self,
        path_item_index: NodeIndex,
        path: &impl PathAnalysis,
    ) -> CgResult<PathRoot> {
        let root = path.extract_path_root();
        let mut current_index = self
            .get_syn_module_index(path_item_index)
            .context(add_context!("Expected module index of syn item."))?;
        // Check path root
        match root.to_string().as_str() {
            "crate" => {
                // module of current crate
                let crate_index =
                    self.get_crate_index(current_index)
                        .context(add_context!(format!(
                            "Expected crate index of module index {:?}",
                            current_index
                        )))?;
                current_index = crate_index;
            }
            "self" => {
                // current module, do nothing
            }
            "super" => {
                // super module
                current_index = self
                    .get_syn_module_index(current_index)
                    .context(add_context!("Expected source index of syn item."))?;
            }
            _ => {
                // check if module points to external or local package dependency
                if self
                    .iter_external_dependencies()
                    .any(|dep_name| root == dep_name)
                {
                    return Ok(PathRoot::ExternalPackage);
                }
                if let Some((lib_crate_index, _)) =
                    self.iter_lib_crates().find(|(_, cf)| root == cf.name)
                {
                    current_index = lib_crate_index;
                } else if let Some((item_index, _)) = self
                    .iter_syn_item_neighbors(current_index)
                    .filter_map(|(n, i)| {
                        ItemName::from(i)
                            .get_ident_in_name_space()
                            .map(|name| (n, name))
                    })
                    .find(|(_, i)| root == *i)
                {
                    // found local item
                    current_index = item_index;
                } else {
                    // could not identify root, probably because of not expanded use group or glob
                    return Ok(PathRoot::PathCouldNotBeParsed);
                }
            }
        }
        Ok(PathRoot::Item(current_index))
    }

    pub fn get_path_leaf(
        &self,
        path_item_index: NodeIndex,
        path: &impl PathAnalysis,
    ) -> TreeResult<PathElement> {
        SourcePathWalker::new(&path.extract_path(), self, path_item_index)
            .into_iter(self)
            .last()
            .context(add_context!("Expected path target."))
            .map_err(|err| err.into())
    }

    pub fn get_use_item_leaf(&self, index_of_use_item: NodeIndex) -> TreeResult<PathElement> {
        if let Some(Item::Use(item_use)) = self.get_syn_item(index_of_use_item) {
            return self.get_path_leaf(index_of_use_item, &item_use.tree);
        }
        Err(anyhow!(add_context!("Expected syn use item")).into())
    }
}

impl<O: CliInput, S> CgData<O, S> {
    pub fn get_challenge_bin_name(&self) -> &str {
        if self.options.input().input == "main" {
            // if main, use crate name for bin name
            self.challenge_package().name.as_str()
        } else {
            self.options.input().input.as_str()
        }
    }

    pub fn get_challenge_bin_crate(&self) -> Option<(NodeIndex, &CrateFile)> {
        let bin_name = self.get_challenge_bin_name();
        self.iter_package_crates(0.into())
            .filter_map(|(n, crate_type, cf)| if !crate_type { Some((n, cf)) } else { None })
            .find(|(_, cf)| cf.name == bin_name)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PathRoot {
    ExternalPackage,
    Item(NodeIndex),
    PathCouldNotBeParsed,
}

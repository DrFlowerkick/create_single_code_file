// functions to navigate the challenge tree

use super::{
    visit::BfsByEdgeType, ChallengeTreeError, CrateFile, EdgeType, LocalPackage, NodeTyp,
    TreeResult,
};
use crate::{
    add_context,
    configuration::CliInput,
    error::CgResult,
    parsing::{get_name_of_item, PathAnalysis},
    CgData,
};

use anyhow::Context;
use petgraph::{graph::NodeIndex, visit::EdgeRef, Direction};
use syn::{Ident, Item};

impl<O, S> CgData<O, S> {
    pub fn challenge_package(&self) -> &LocalPackage {
        if let NodeTyp::LocalPackage(ref package) = self.tree.node_weight(0.into()).unwrap() {
            return package;
        }
        unreachable!("Challenge package is created at instantiation of CgDate and should always be at index 0.");
    }

    pub fn link_to_package(&mut self, source: NodeIndex, target: NodeIndex) {
        self.tree.add_edge(source, target, EdgeType::Dependency);
    }

    pub fn get_local_package(&self, node: NodeIndex) -> TreeResult<&LocalPackage> {
        if let NodeTyp::LocalPackage(dependency) = self
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
        if let NodeTyp::BinCrate(crate_file) = self
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
        if let NodeTyp::LibCrate(crate_file) = self
            .tree
            .node_weight(node)
            .ok_or_else(|| ChallengeTreeError::IndexError(node))?
        {
            Ok(crate_file)
        } else {
            Err(ChallengeTreeError::NotLibraryCrate(node))
        }
    }

    fn iter_packages(&self) -> impl Iterator<Item = (NodeIndex, &NodeTyp)> {
        BfsByEdgeType::new(&self.tree, 0.into(), EdgeType::Dependency)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .fuse()
    }

    pub fn iter_local_packages(&self) -> impl Iterator<Item = (NodeIndex, &LocalPackage)> {
        self.iter_packages().filter_map(|(n, w)| match w {
            NodeTyp::LocalPackage(package) => Some((n, package)),
            NodeTyp::ExternalSupportedPackage(_) | NodeTyp::ExternalUnsupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    pub fn iter_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &NodeTyp)> {
        // skip first element, which is root of tree and therefore not a dependency
        self.iter_packages().skip(1)
    }

    pub fn iter_accepted_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.iter_dependencies().filter_map(|(n, w)| match w {
            NodeTyp::LocalPackage(local_package) => Some((n, local_package.name.as_str())),
            NodeTyp::ExternalSupportedPackage(name) => Some((n, name.as_str())),
            NodeTyp::ExternalUnsupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    pub fn iter_unsupported_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.iter_dependencies().filter_map(|(n, w)| match w {
            NodeTyp::ExternalUnsupportedPackage(name) => Some((n, name.as_str())),
            NodeTyp::LocalPackage(_) | NodeTyp::ExternalSupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    pub fn iter_external_dependencies(&self) -> impl Iterator<Item = &str> {
        // include elements of rust libraries in iterator
        self.iter_dependencies()
            .filter_map(|(_, w)| match w {
                NodeTyp::ExternalSupportedPackage(name)
                | NodeTyp::ExternalUnsupportedPackage(name) => Some(name.as_str()),
                NodeTyp::LocalPackage(_) => None,
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
                NodeTyp::BinCrate(bin_crate_file) => Some((n, false, bin_crate_file)),
                NodeTyp::LibCrate(lib_crate_file) => Some((n, true, lib_crate_file)),
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

    pub fn iter_syn_neighbors(&self, node: NodeIndex) -> impl Iterator<Item = (NodeIndex, &Item)> {
        self.tree
            .edges_directed(node, Direction::Outgoing)
            .filter(|e| *e.weight() == EdgeType::Syn)
            .map(|e| e.target())
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| match w {
                NodeTyp::SynItem(item) => Some((n, item)),
                _ => unreachable!("All syn edges must end in SynItem nodes."),
            })
    }

    pub fn iter_syn_items(&self, node: NodeIndex) -> impl Iterator<Item = (NodeIndex, &Item)> {
        BfsByEdgeType::new(&self.tree, node, EdgeType::Syn)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| match w {
                NodeTyp::SynItem(syn_item) => Some((n, syn_item)),
                _ => None,
            })
            .fuse()
    }

    fn get_parent_index_by_edge_type(
        &self,
        node: NodeIndex,
        edge_type: EdgeType,
    ) -> Option<NodeIndex> {
        self.tree
            .edges_directed(node, Direction::Incoming)
            .find(|e| *e.weight() == edge_type)
            .map(|e| e.source())
    }

    pub fn get_syn_item_module_index(&self, node: NodeIndex) -> Option<NodeIndex> {
        let module_index = self.get_parent_index_by_edge_type(node, EdgeType::Syn);
        if let Some(NodeTyp::SynImplItem(_)) = self.tree.node_weight(node) {
            if let Some(mi) = module_index {
                return self.get_syn_item_module_index(mi);
            }
        }
        module_index
    }

    pub fn get_syn_item(&self, node: NodeIndex) -> Option<&Item> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeTyp::SynItem(item) => Some(item),
            _ => None,
        })
    }

    pub fn get_name_of_crate_or_module(&self, node: NodeIndex) -> Option<String> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeTyp::SynItem(item) => {
                if let Item::Mod(item_mod) = item {
                    Some(item_mod.ident.to_string())
                } else {
                    None
                }
            }
            NodeTyp::BinCrate(crate_file) | NodeTyp::LibCrate(crate_file) => {
                Some(crate_file.name.to_owned())
            }
            _ => None,
        })
    }

    pub fn get_crate_index(&self, node: NodeIndex) -> Option<NodeIndex> {
        if let Some(node_weight) = self.tree.node_weight(node) {
            match node_weight {
                NodeTyp::BinCrate(_) | NodeTyp::LibCrate(_) => return Some(node),
                NodeTyp::SynItem(_) | NodeTyp::SynImplItem(_) => {
                    if let Some(parent) = self.get_parent_index_by_edge_type(node, EdgeType::Syn) {
                        return self.get_crate_index(parent);
                    }
                }
                _ => (),
            }
        }
        None
    }

    pub fn is_crate_or_module(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return match node_weight {
                NodeTyp::BinCrate(_) | NodeTyp::LibCrate(_) | NodeTyp::SynItem(Item::Mod(_)) => {
                    true
                }
                _ => false,
            };
        }
        false
    }

    pub fn is_item_descendant_of_or_same_module(
        &self,
        item_index: NodeIndex,
        module_index: NodeIndex,
    ) -> bool {
        if item_index == module_index {
            return true;
        }
        if let Some(parent_index) = self.get_parent_index_by_edge_type(item_index, EdgeType::Syn) {
            return self.is_item_descendant_of_or_same_module(parent_index, module_index);
        }
        false
    }

    pub fn iter_syn_neighbors_without_semantic_link(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &Item)> {
        self.iter_syn_neighbors(node).filter(move |(target, _)| {
            !self
                .tree
                .edges_connecting(node, *target)
                .any(|e| *e.weight() == EdgeType::Semantic)
        })
    }

    pub fn get_path_target(
        &self,
        path_item_index: NodeIndex,
        path: &impl PathAnalysis,
    ) -> CgResult<PathTarget> {
        if let Some(extracted_path) = path.extract_path() {
            let mut module_index = self
                .get_syn_item_module_index(path_item_index)
                .context(add_context!("Expected source index of syn item."))?;
            // walk trough the path
            for (seg_index, segment) in extracted_path.segments.iter().enumerate() {
                match segment.to_string().as_str() {
                    "crate" => {
                        // module of current crate
                        let crate_index =
                            self.get_crate_index(module_index)
                                .context(add_context!(format!(
                                    "Expected crate index of module index {:?}",
                                    module_index
                                )))?;
                        module_index = crate_index;
                    }
                    "self" => {
                        // current module, do nothing
                    }
                    "super" => {
                        // super module
                        module_index = self
                            .get_syn_item_module_index(module_index)
                            .context(add_context!("Expected source index of syn item."))?;
                    }
                    _ => {
                        if seg_index == 0 {
                            // check if module points to external or local package dependency
                            if self
                                .iter_external_dependencies()
                                .any(|dep_name| segment == dep_name)
                            {
                                return Ok(PathTarget::ExternalPackage);
                            }
                            if let Some((lib_crate_index, _)) =
                                self.iter_lib_crates().find(|(_, cf)| *segment == cf.name)
                            {
                                module_index = lib_crate_index;
                                continue;
                            }
                        }
                        if seg_index < extracted_path.segments.len() - 1 {
                            // segments of path before target item
                            if let Some((sub_module_index, _)) = self
                                .iter_syn_neighbors(module_index)
                                .filter_map(|(n, i)| match i {
                                    Item::Mod(mod_item) => Some((n, mod_item.ident.to_string())),
                                    _ => None,
                                })
                                .find(|(_, m)| segment == m)
                            {
                                // found local module
                                module_index = sub_module_index;
                            } else if let Some((use_module_index, _, use_tree)) = self
                                .iter_syn_neighbors(module_index)
                                .filter_map(|(n, i)| match i {
                                    Item::Use(item_use) => get_name_of_item(i)
                                        .extract_ident()
                                        .map(|ident| (n, ident, &item_use.tree)),
                                    _ => None,
                                })
                                .find(|(_, m, _)| segment == m)
                            {
                                // found reimported module
                                match self.get_path_target(use_module_index, use_tree)? {
                                    PathTarget::ExternalPackage => {
                                        return Ok(PathTarget::ExternalPackage)
                                    }
                                    PathTarget::Glob(_) => {
                                        unreachable!(
                                            "filter_map use statements which end on name or rename"
                                        );
                                    }
                                    PathTarget::Item(item_index) => {
                                        // Item must be module
                                        assert!(matches!(
                                            self.get_syn_item(item_index),
                                            Some(Item::Mod(_))
                                        ));
                                        module_index = item_index;
                                    }
                                    PathTarget::ItemRenamed(item_index, _) => {
                                        // Renamed item must be module
                                        assert!(matches!(
                                            self.get_syn_item(item_index),
                                            Some(Item::Mod(_))
                                        ));
                                        module_index = item_index;
                                    }
                                    PathTarget::PathCouldNotBeParsed => {
                                        return Ok(PathTarget::PathCouldNotBeParsed)
                                    }
                                }
                            } else {
                                // could not find module of segment
                                return Ok(PathTarget::PathCouldNotBeParsed);
                            }
                        } else {
                            // get target index of path
                            if extracted_path.glob {
                                return Ok(PathTarget::Glob(module_index));
                            }
                            // search for item in tree und fetch it's Index
                            let (item_index, _) = self
                                .iter_syn_neighbors(module_index)
                                .filter_map(|(n, i)| {
                                    get_name_of_item(i).extract_ident().map(|ident| (n, ident))
                                })
                                .find(|(_, n)| segment == n)
                                .context(add_context!(format!(
                                    "Expected item_index of '{:?}'",
                                    segment
                                )))?;
                            if let Some(rename) = extracted_path.rename {
                                return Ok(PathTarget::ItemRenamed(item_index, rename));
                            }
                            return Ok(PathTarget::Item(item_index));
                        }
                    }
                }
            }
        }
        Ok(PathTarget::PathCouldNotBeParsed)
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

    pub fn get_crate_indices(&self, reverse: bool) -> TreeResult<Vec<NodeIndex>> {
        // get challenge bin and all lib crate indices
        let (bin_crate_index, _) = self
            .get_challenge_bin_crate()
            .context(add_context!("Expected challenge bin."))?;
        let mut crate_indices: Vec<NodeIndex> = self.iter_lib_crates().map(|(n, _)| n).collect();
        if reverse {
            // indices from end of dependency tree to challenge bin crate
            crate_indices.reverse();
            crate_indices.push(bin_crate_index);
        } else {
            // indices from challenge bin crate to end of dependency tree
            crate_indices.insert(0, bin_crate_index);
        }
        Ok(crate_indices)
    }
}

#[derive(Debug)]
pub enum PathTarget {
    ExternalPackage,
    Glob(NodeIndex),
    Item(NodeIndex),
    ItemRenamed(NodeIndex, Ident),
    PathCouldNotBeParsed,
}

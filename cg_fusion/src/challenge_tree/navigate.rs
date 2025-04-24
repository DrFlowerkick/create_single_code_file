// functions to navigate the challenge tree

use super::{
    ChallengeTreeError, EdgeType, LocalPackage, NodeType, SrcFile, TreeResult,
    walkers::{PathElement, SourcePathWalker},
};
use crate::{
    CgData, add_context,
    configuration::CgCli,
    parsing::{IdentCollector, ItemName, SourcePath},
    utilities::clean_absolute_utf8,
};

use anyhow::{Context, anyhow};
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::{Direction, stable_graph::NodeIndex, visit::EdgeRef};
use proc_macro2::Span;
use std::collections::{HashSet, VecDeque};
use syn::{Ident, ImplItem, Item, TraitItem, spanned::Spanned, visit::Visit};

impl<O, S> CgData<O, S> {
    pub(crate) fn challenge_package(&self) -> &LocalPackage {
        if let NodeType::LocalPackage(package) = self.tree.node_weight(0.into()).unwrap() {
            return package;
        }
        unreachable!(
            "Challenge package is created at instantiation of CgDate and should always be at index 0."
        );
    }

    pub(crate) fn get_local_package(&self, node: NodeIndex) -> TreeResult<&LocalPackage> {
        match self
            .tree
            .node_weight(node)
            .ok_or_else(|| ChallengeTreeError::IndexError(node))?
        {
            NodeType::LocalPackage(dependency) => Ok(dependency),
            _ => Err(ChallengeTreeError::NotLocalPackage(node)),
        }
    }

    pub(crate) fn get_binary_crate(&self, node: NodeIndex) -> TreeResult<&SrcFile> {
        match self
            .tree
            .node_weight(node)
            .ok_or_else(|| ChallengeTreeError::IndexError(node))?
        {
            NodeType::BinCrate(src_file) => Ok(src_file),
            _ => Err(ChallengeTreeError::NotBinaryCrate(node)),
        }
    }

    pub(crate) fn get_library_crate(&self, node: NodeIndex) -> TreeResult<&SrcFile> {
        match self
            .tree
            .node_weight(node)
            .ok_or_else(|| ChallengeTreeError::IndexError(node))?
        {
            NodeType::LibCrate(src_file) => Ok(src_file),
            _ => Err(ChallengeTreeError::NotLibraryCrate(node)),
        }
    }

    pub(crate) fn get_parent_index_by_edge_type(
        &self,
        node: NodeIndex,
        edge_type: EdgeType,
    ) -> Option<NodeIndex> {
        self.tree
            .edges_directed(node, Direction::Incoming)
            .filter(|e| *e.weight() == edge_type)
            .map(|e| e.source())
            .next()
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

    pub(crate) fn get_path_module(&self, node: NodeIndex, path: SourcePath) -> Option<NodeIndex> {
        let mut source_path_walker = SourcePathWalker::new(path, node);
        let mut path_nodes: Vec<NodeIndex> = Vec::new();
        while let Some(path_element) = source_path_walker.next(self) {
            match path_element {
                PathElement::Item(item_node) | PathElement::ItemRenamed(item_node, _) => {
                    path_nodes.push(item_node);
                }
                PathElement::Glob(glob_module) => return Some(glob_module),
                PathElement::Group | PathElement::PathCouldNotBeParsed => return None,
                PathElement::ExternalGlob(_) | PathElement::ExternalItem(_) => {
                    if path_nodes.is_empty() {
                        return self.get_syn_module_index(node);
                    }
                }
            }
        }
        if path_nodes.is_empty() {
            return None;
        }
        let leaf_node = path_nodes[path_nodes.len() - 1];
        if self.is_crate_or_module(leaf_node) {
            return Some(leaf_node);
        }
        if path_nodes.len() == 1 {
            return None;
        }
        Some(path_nodes[path_nodes.len() - 2])
    }

    pub(crate) fn get_syn_item(&self, node: NodeIndex) -> Option<&Item> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeType::SynItem(item) => Some(item),
            _ => None,
        })
    }

    pub(crate) fn get_syn_impl_item(&self, node: NodeIndex) -> Option<&ImplItem> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeType::SynImplItem(impl_item) => Some(impl_item),
            _ => None,
        })
    }

    pub(crate) fn clone_syn_item(&self, node: NodeIndex) -> Option<Item> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeType::SynItem(item) => Some(item.clone()),
            _ => None,
        })
    }

    pub(crate) fn clone_syn_impl_item(&self, node: NodeIndex) -> Option<ImplItem> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeType::SynImplItem(impl_item) => Some(impl_item.clone()),
            _ => None,
        })
    }

    pub(crate) fn clone_syn_trait_item(&self, node: NodeIndex) -> Option<TraitItem> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeType::SynTraitItem(trait_item) => Some(trait_item.clone()),
            _ => None,
        })
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
            NodeType::BinCrate(src_file) | NodeType::LibCrate(src_file) => {
                Some(src_file.name.to_owned())
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

    pub(crate) fn get_crate_path_nodes(&self, node: NodeIndex) -> Vec<NodeIndex> {
        let Some(node_type) = self.tree.node_weight(node) else {
            return Vec::new();
        };
        let mut path_nodes: Vec<NodeIndex> = match node_type {
            NodeType::BinCrate(_) | NodeType::LibCrate(_) => return vec![node],
            NodeType::SynItem(_) | NodeType::SynImplItem(_) | NodeType::SynTraitItem(_) => {
                if let Some(module) = self.get_syn_module_index(node) {
                    self.get_crate_path_nodes(module)
                } else {
                    return Vec::new();
                }
            }
            _ => return Vec::new(),
        };
        path_nodes.push(node);
        path_nodes
    }

    pub(crate) fn get_ident(&self, node: NodeIndex) -> Option<Ident> {
        let node_type = self.tree.node_weight(node)?;
        match node_type {
            NodeType::ExternalSupportedPackage(_)
            | NodeType::ExternalUnsupportedPackage(_)
            | NodeType::LocalPackage(_) => None,
            NodeType::BinCrate(src_file)
            | NodeType::LibCrate(src_file)
            | NodeType::Module(src_file) => Some(Ident::new(&src_file.name, Span::call_site())),
            NodeType::SynItem(item) => ItemName::from(item).get_ident_in_name_space(),
            NodeType::SynImplItem(impl_item) => ItemName::from(impl_item).get_ident_in_name_space(),
            NodeType::SynTraitItem(trait_item) => {
                ItemName::from(trait_item).get_ident_in_name_space()
            }
        }
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
            NodeType::BinCrate(src_file) => Ok(format!("{} (binary crate)", src_file.name)),
            NodeType::LibCrate(src_file) => Ok(format!("{} (library crate)", src_file.name)),
            NodeType::Module(src_file) => Ok(format!("{} (module src file)", src_file.name)),
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

    pub(crate) fn is_external(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(
                node_weight,
                NodeType::ExternalSupportedPackage(_) | NodeType::ExternalUnsupportedPackage(_)
            );
        }
        false
    }

    pub(crate) fn is_crate(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(node_weight, NodeType::BinCrate(_) | NodeType::LibCrate(_));
        }
        false
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
                    | NodeType::SynTraitItem(_)
            );
        }
        false
    }

    pub(crate) fn is_syn_impl_item(&self, node: NodeIndex) -> bool {
        if let Some(node_weight) = self.tree.node_weight(node) {
            return matches!(node_weight, NodeType::SynImplItem(_));
        }
        false
    }

    pub(crate) fn get_syn_impl_item_self_type_node(&self, node: NodeIndex) -> Option<NodeIndex> {
        if !self.is_syn_impl_item(node) {
            return None;
        }
        if let Some(impl_block_index) = self.get_parent_index_by_edge_type(node, EdgeType::Syn) {
            return self.get_parent_index_by_edge_type(impl_block_index, EdgeType::Implementation);
        }
        None
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

    pub(crate) fn is_unambiguous_impl_item(&self, node: NodeIndex) -> bool {
        if let Some(impl_item) = self.get_syn_impl_item(node) {
            let Some(impl_item_name) = ItemName::from(impl_item).get_ident_in_name_space() else {
                return false;
            };
            return !self
                .iter_crates()
                .flat_map(|(n, ..)| self.iter_syn(n))
                .any(|(n, nt)| {
                    if let NodeType::SynImplItem(ii) = nt {
                        if let Some(iin) = ItemName::from(ii).get_ident_in_name_space() {
                            node != n && impl_item_name == iin
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                });
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
        path: SourcePath,
    ) -> TreeResult<PathElement> {
        SourcePathWalker::new(path, path_item_index)
            .into_iter(self)
            .last()
            .context(add_context!("Expected path target."))
            .map_err(|err| err.into())
    }

    pub(crate) fn get_path_root(
        &self,
        path_item_index: NodeIndex,
        path: SourcePath,
    ) -> TreeResult<PathElement> {
        SourcePathWalker::new(path, path_item_index)
            .next(self)
            .context(add_context!("Expected path target."))
            .map_err(|err| err.into())
    }

    pub(crate) fn get_use_item_leaf(
        &self,
        index_of_use_item: NodeIndex,
    ) -> TreeResult<PathElement> {
        if let Some(Item::Use(item_use)) = self.get_syn_item(index_of_use_item) {
            return self.get_path_leaf(index_of_use_item, SourcePath::from(&item_use.tree));
        }
        Err(anyhow!(add_context!("Expected syn use item")).into())
    }

    pub(crate) fn get_possible_usage_of_impl_item_in_required_items(
        &self,
        node: NodeIndex,
    ) -> Vec<(NodeIndex, Span, Ident)> {
        let mut possible_usage: Vec<(NodeIndex, Span, Ident)> = Vec::new();
        let item_name = match self.tree.node_weight(node) {
            Some(NodeType::SynImplItem(impl_item)) => {
                if let Some(name) = ItemName::from(impl_item).get_ident_in_name_space() {
                    name.to_string()
                } else {
                    return possible_usage;
                }
            }
            _ => return possible_usage,
        };
        let mut ident_collector = IdentCollector::new(item_name);
        possible_usage = self
            .iter_items_required_by_challenge()
            .filter_map(|(n, nt)| match nt {
                NodeType::SynItem(item) => {
                    ident_collector.visit_item(item);
                    ident_collector
                        .extract_collector()
                        .map(|c| (n, item.span(), c))
                }
                NodeType::SynImplItem(impl_item) => {
                    ident_collector.visit_impl_item(impl_item);
                    ident_collector
                        .extract_collector()
                        .map(|c| (n, impl_item.span(), c))
                }
                NodeType::SynTraitItem(trait_item) => {
                    ident_collector.visit_trait_item(trait_item);
                    ident_collector
                        .extract_collector()
                        .map(|c| (n, trait_item.span(), c))
                }
                _ => None,
            })
            .flat_map(|(n, s, c)| c.into_iter().map(move |i| (n, s, i)))
            .collect();
        possible_usage
    }

    pub(crate) fn get_src_file_containing_item(&self, node: NodeIndex) -> Option<&SrcFile> {
        self.get_syn_module_index(node).and_then(|module_or_crate| {
            match self.tree.node_weight(module_or_crate) {
                Some(NodeType::BinCrate(src_file)) | Some(NodeType::LibCrate(src_file)) => {
                    Some(src_file)
                }
                Some(NodeType::SynItem(Item::Mod(_))) => {
                    if let Some(module_src_node) =
                        self.get_parent_index_by_edge_type(module_or_crate, EdgeType::Module)
                    {
                        match self.tree.node_weight(module_src_node) {
                            Some(NodeType::Module(src_file)) => Some(src_file),
                            _ => None,
                        }
                    } else {
                        self.get_src_file_containing_item(module_or_crate)
                    }
                }
                _ => None,
            }
        })
    }

    pub(crate) fn get_sorted_mod_content(&self, mod_index: NodeIndex) -> TreeResult<Vec<Item>> {
        let item_order = self
            .item_order
            .get(&mod_index)
            .context(add_context!("Expected item order of mod."))?;
        let sorted_mod_content: Vec<Item> = item_order
            .iter()
            .filter_map(|n| self.get_syn_item(*n).cloned())
            .collect();
        Ok(sorted_mod_content)
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

    pub(crate) fn get_challenge_bin_crate(&self) -> Option<(NodeIndex, &SrcFile)> {
        let bin_name = self.get_challenge_bin_name();
        self.iter_package_crates(0.into())
            .find_map(|(n, crate_type, cf)| (!crate_type && cf.name == bin_name).then_some((n, cf)))
    }

    pub(crate) fn get_challenge_lib_crate(&self) -> Option<(NodeIndex, &SrcFile)> {
        let bin_name = self.get_challenge_bin_name();
        self.iter_package_crates(0.into())
            .find_map(|(n, crate_type, cf)| (crate_type && cf.name == bin_name).then_some((n, cf)))
    }

    pub(crate) fn is_challenge_item(&self, node: NodeIndex) -> bool {
        let Some(node_crate) = self.get_crate_index(node) else {
            return false;
        };
        if let Some((challenge_bin_crate, _)) = self.get_challenge_bin_crate() {
            if node_crate == challenge_bin_crate {
                return true;
            }
        }
        if let Some((challenge_lib_crate, _)) = self.get_challenge_lib_crate() {
            if node_crate == challenge_lib_crate {
                return true;
            }
        };
        false
    }

    pub(crate) fn get_required_crates_and_modules_sorted_by_relevance(
        &self,
    ) -> TreeResult<VecDeque<NodeIndex>> {
        let mut seen: HashSet<NodeIndex> = HashSet::new();
        let (bin_node, _) = self
            .get_challenge_bin_crate()
            .context(add_context!("Expected challenge bin."))?;
        let sorted_list =
            self.recursive_list_of_required_crates_and_modules(bin_node, &mut seen)?;
        Ok(sorted_list)
    }

    fn recursive_list_of_required_crates_and_modules(
        &self,
        node: NodeIndex,
        seen: &mut HashSet<NodeIndex>,
    ) -> TreeResult<VecDeque<NodeIndex>> {
        let mut sorted_list: VecDeque<NodeIndex> = VecDeque::new();
        if seen.insert(node) {
            // collect crate dependencies
            let current_crate_node = self
                .get_crate_index(node)
                .context(add_context!("Expected crate index."))?;
            for crate_node in self
                .iter_syn_item_neighbors(node)
                .filter(|(n, _)| self.is_required_by_challenge(*n))
                .filter_map(|(n, i)| match i {
                    Item::Use(item_use) => match self.get_path_root(n, item_use.into()) {
                        Ok(PathElement::Item(root_node)) => {
                            self.get_crate_index(root_node).and_then(|root_crate| {
                                (root_crate != current_crate_node).then_some(root_crate)
                            })
                        }
                        _ => None,
                    },
                    _ => None,
                })
            {
                let crate_dependencies =
                    self.recursive_list_of_required_crates_and_modules(crate_node, seen)?;
                sorted_list.extend(crate_dependencies);
            }

            // collect modules of crate
            let mut modules: VecDeque<NodeIndex> = VecDeque::new();
            for mod_node in self
                .iter_syn_item_neighbors(node)
                .filter(|(n, _)| self.is_required_by_challenge(*n))
                .filter_map(|(n, i)| match i {
                    Item::Mod(_) => Some(n),
                    _ => None,
                })
            {
                let sub_modules =
                    self.recursive_list_of_required_crates_and_modules(mod_node, seen)?;
                modules.extend(sub_modules);
            }
            // push modules to front
            if !modules.is_empty() {
                modules.extend(sorted_list);
                sorted_list = modules;
            }

            // add current node to list
            sorted_list.push_front(node);
        }

        Ok(sorted_list)
    }

    pub(crate) fn get_fusion_file_name(&self) -> String {
        match self.options.output().filename {
            Some(ref name) => name.to_owned(),
            _ => {
                format!("fusion_of_{}", self.challenge_package().name)
            }
        }
    }

    pub(crate) fn get_fusion_file_path(&self) -> TreeResult<Utf8PathBuf> {
        let fusion_file_name = format!("{}.rs", self.get_fusion_file_name());
        let fusion_bin_dir = self.challenge_package().path.join("src/bin/");
        let fusion_bin_dir = clean_absolute_utf8(fusion_bin_dir)?;
        Ok(fusion_bin_dir.join(&fusion_file_name))
    }

    pub(crate) fn get_fusion_bin_crate(&self) -> Option<(NodeIndex, &SrcFile)> {
        let bin_name = self.get_fusion_file_name();
        self.iter_package_crates(0.into())
            .filter_map(|(n, crate_type, cf)| if !crate_type { Some((n, cf)) } else { None })
            .find(|(_, cf)| cf.name == bin_name)
    }
}

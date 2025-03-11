// functions to expand the challenge tree
use super::{
    ChallengeTreeError, EdgeType, FusedDepPathFolder, LocalPackage, NodeType, SourcePathWalker,
    SrcFile, SynReferenceMapper, TreeResult,
};
use crate::{
    CgData, add_context,
    challenge_tree::PathElement,
    configuration::CgCli,
    parsing::{ItemName, SourcePath, ToTokensExt, UseTreeExt, load_syntax},
};

use anyhow::{Context, anyhow};
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::stable_graph::NodeIndex;
use proc_macro2::Span;
use std::collections::HashSet;
use std::fs;
use syn::{
    Ident, ImplItem, Item, ItemMod, TraitItem, UsePath, UseTree, Visibility, fold::Fold, token,
    visit::Visit,
};

impl<O: CgCli, S> CgData<O, S> {
    pub(crate) fn add_local_package(
        &mut self,
        source: NodeIndex,
        package: LocalPackage,
    ) -> NodeIndex {
        let package_path = package.path.to_owned();
        let package_index = self.tree.add_node(NodeType::LocalPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
        if self.options.verbose() {
            println!(
                "Adding '{}' at path '{}' to tree.",
                self.get_verbose_name_of_tree_node(package_index).unwrap(),
                package_path
            );
        }
        package_index
    }

    pub(crate) fn link_to_package(&mut self, source: NodeIndex, target: NodeIndex) {
        self.tree.add_edge(source, target, EdgeType::Dependency);
    }

    pub(crate) fn add_external_supported_package(
        &mut self,
        source: NodeIndex,
        package: String,
    ) -> NodeIndex {
        let package_index = self
            .tree
            .add_node(NodeType::ExternalSupportedPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
        if self.options.verbose() {
            println!(
                "Adding '{}' to tree.",
                self.get_verbose_name_of_tree_node(package_index).unwrap()
            );
        }
        package_index
    }

    pub(crate) fn add_external_unsupported_package(
        &mut self,
        source: NodeIndex,
        package: String,
    ) -> NodeIndex {
        let package_index = self
            .tree
            .add_node(NodeType::ExternalUnsupportedPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
        if self.options.verbose() {
            println!(
                "Adding '{}' to tree.",
                self.get_verbose_name_of_tree_node(package_index).unwrap()
            );
        }
        package_index
    }

    pub(crate) fn add_binary_crate_to_package(
        &mut self,
        package_node_index: NodeIndex,
        name: String,
    ) -> TreeResult<NodeIndex> {
        // get bin path from metadata
        let path = self
            .get_local_package(package_node_index)?
            .metadata
            .get_binary_target_of_root_package(name.as_str())?
            .src_path
            .to_owned();

        // load source code
        let code = fs::read_to_string(&path)?;
        // get syntax of src file
        let syntax = load_syntax(&code)?;
        // generate node value
        let crate_file = SrcFile {
            name,
            path: path.to_owned(),
            shebang: syntax.shebang,
            attrs: syntax.attrs,
        };

        let crate_node_index = self.tree.add_node(NodeType::BinCrate(crate_file));
        self.tree
            .add_edge(package_node_index, crate_node_index, EdgeType::Crate);

        if self.options.verbose() {
            println!(
                "Adding '{}' at path '{}' to tree.",
                self.get_verbose_name_of_tree_node(crate_node_index)?,
                path
            );
        }

        Ok(crate_node_index)
    }

    pub(crate) fn add_library_crate_to_package(
        &mut self,
        package_node_index: NodeIndex,
    ) -> TreeResult<Option<NodeIndex>> {
        // get bin path from metadata
        match self
            .get_local_package(package_node_index)?
            .metadata
            .get_library_target_of_root_package()?
        {
            Some(target) => {
                // load source code
                let code = fs::read_to_string(&target.src_path)?;
                // get syntax of src file
                let syntax = load_syntax(&code)?;
                // generate node value
                let crate_file = SrcFile {
                    name: target.name.to_owned(),
                    path: target.src_path.to_owned(),
                    shebang: syntax.shebang,
                    attrs: syntax.attrs,
                };

                let path = target.src_path.to_owned();

                let crate_node_index = self.tree.add_node(NodeType::LibCrate(crate_file));
                self.tree
                    .add_edge(package_node_index, crate_node_index, EdgeType::Crate);

                if self.options.verbose() {
                    println!(
                        "Adding '{}' at path '{}' to tree.",
                        self.get_verbose_name_of_tree_node(crate_node_index)?,
                        path
                    );
                }

                Ok(Some(crate_node_index))
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn add_syn_item(
        &mut self,
        item: &Item,
        dir_path: &Utf8PathBuf,
        source_index: NodeIndex,
    ) -> TreeResult<NodeIndex> {
        // add item to tree
        let item_index: NodeIndex = self.tree.add_node(NodeType::SynItem(item.to_owned()));
        self.tree.add_edge(source_index, item_index, EdgeType::Syn);

        match item {
            // if item is module, add content of module to tree
            Item::Mod(_) => {
                self.add_syn_item_mod(dir_path, item_index)?;
            }
            // if item is impl, add content of impl to tree
            Item::Impl(_) => {
                if self.options.verbose() {
                    println!("Adding '{}' to tree.", ItemName::from(item));
                }
                self.add_syn_item_impl(item_index)?;
            }
            // if item is impl, add content of impl to tree
            Item::Trait(_) => {
                if self.options.verbose() {
                    println!("Adding '{}' to tree.", ItemName::from(item));
                }
                self.add_syn_item_trait(item_index)?;
            }
            // if item is use statement, check if an ident exist (not the case if group or glob)
            Item::Use(item_use) => {
                if self.options.verbose() {
                    let use_item_name = ItemName::from(item);
                    if use_item_name.get_ident_in_name_space().is_some() {
                        println!("Adding '{}' to tree.", use_item_name);
                    } else {
                        // use statement with group or glob
                        println!(
                            "Adding '{}' (Use) to tree.",
                            item_use.to_trimmed_token_string()
                        );
                    }
                }
            }
            _ => {
                if self.options.verbose() {
                    println!("Adding '{}' to tree.", ItemName::from(item));
                }
            }
        }
        Ok(item_index)
    }

    fn add_syn_item_mod(
        &mut self,
        dir_path: &Utf8PathBuf,
        item_mod_index: NodeIndex,
    ) -> TreeResult<()> {
        let (items, mod_src_file) = match self.tree.node_weight_mut(item_mod_index) {
            Some(NodeType::SynItem(Item::Mod(item_mod))) => {
                match item_mod.content.take() {
                    Some(mod_content) => {
                        // with take() mod_content of item_mod is set to None
                        if self.options.verbose() {
                            println!(
                                "Adding inline '{}' to tree.",
                                self.get_verbose_name_of_tree_node(item_mod_index)?
                            );
                        }
                        (mod_content.1, None)
                    }
                    _ => {
                        // set module directory
                        let mod_dir = dir_path.join(item_mod.ident.to_string());
                        // set module filename
                        let mut path = mod_dir.join("mod.rs");
                        // module is either 'module_name.rs' or 'module_name/mod.rs'
                        if !path.is_file() {
                            path = mod_dir.clone();
                            path.set_extension("rs");
                            if !path.is_file() {
                                Err(anyhow!(add_context!("Unexpected module file path error.")))?;
                            }
                        }
                        // get syntax of src file
                        let code = fs::read_to_string(&path)?;
                        let mod_syntax = load_syntax(&code)?;
                        let src_file = SrcFile {
                            name: item_mod.ident.to_string(),
                            path,
                            shebang: mod_syntax.shebang.to_owned(),
                            attrs: mod_syntax.attrs.to_owned(),
                        };
                        (mod_syntax.items, Some((src_file, mod_dir)))
                    }
                }
            }
            _ => {
                return Err(anyhow!(add_context!("Expecting item mod.")).into());
            }
        };

        let mut item_order: Vec<NodeIndex> = Vec::new();
        match mod_src_file {
            Some((src_file, mod_dir)) => {
                // add src file of module to tree
                if self.options.verbose() {
                    println!(
                        "Adding module src file '{}' at path '{}' to tree.",
                        self.get_verbose_name_of_tree_node(item_mod_index)?,
                        src_file.path,
                    );
                }
                let mod_node_index = self.tree.add_node(NodeType::Module(src_file));
                self.tree
                    .add_edge(mod_node_index, item_mod_index, EdgeType::Module);

                // add items of module src file to tree
                for content_item in items.iter() {
                    item_order.push(self.add_syn_item(content_item, &mod_dir, item_mod_index)?);
                }
            }
            _ => {
                for content_item in items.iter() {
                    item_order.push(self.add_syn_item(content_item, dir_path, item_mod_index)?);
                }
            }
        }
        self.item_order.insert(item_mod_index, item_order);

        Ok(())
    }

    fn add_syn_item_impl(&mut self, item_impl_index: NodeIndex) -> TreeResult<()> {
        let items = match self.tree.node_weight_mut(item_impl_index) {
            Some(NodeType::SynItem(Item::Impl(item_impl))) => {
                // mem::take takes all items from item_impl.items, leaving it empty
                std::mem::take(&mut item_impl.items)
            }
            _ => {
                return Err(anyhow!(add_context!("Expected impl item.")).into());
            }
        };

        // Add impl items
        let mut item_order: Vec<NodeIndex> = Vec::new();
        for impl_item in items.iter() {
            let impl_item_index = self
                .tree
                .add_node(NodeType::SynImplItem(impl_item.to_owned()));
            self.tree
                .add_edge(item_impl_index, impl_item_index, EdgeType::Syn);
            item_order.push(impl_item_index);
            if self.options.verbose() {
                println!(
                    "Adding '{}' to tree.",
                    self.get_verbose_name_of_tree_node(impl_item_index)?
                );
            }
        }
        self.item_order.insert(item_impl_index, item_order);
        Ok(())
    }

    fn add_syn_item_trait(&mut self, item_trait_index: NodeIndex) -> TreeResult<()> {
        let items = match self.tree.node_weight_mut(item_trait_index) {
            Some(NodeType::SynItem(Item::Trait(trait_impl))) => {
                // mem::take takes all items from item_impl.items, leaving it empty
                std::mem::take(&mut trait_impl.items)
            }
            _ => {
                return Err(anyhow!(add_context!("Expected trait item.")).into());
            }
        };

        // Add trait items
        let mut item_order: Vec<NodeIndex> = Vec::new();
        for trait_item in items.iter() {
            let trait_item_index = self
                .tree
                .add_node(NodeType::SynTraitItem(trait_item.to_owned()));
            self.tree
                .add_edge(item_trait_index, trait_item_index, EdgeType::Syn);
            item_order.push(trait_item_index);
            if self.options.verbose() {
                println!(
                    "Adding '{}' to tree.",
                    self.get_verbose_name_of_tree_node(trait_item_index)?
                );
            }
        }
        self.item_order.insert(item_trait_index, item_order);
        Ok(())
    }

    pub(crate) fn add_implementation_link(
        &mut self,
        source: NodeIndex,
        syn_impl_item_index: NodeIndex,
    ) -> TreeResult<()> {
        if !self.is_source_item(source) {
            return Err(ChallengeTreeError::NotCrateOrSyn(source));
        }
        if !self.is_source_item(syn_impl_item_index) {
            return Err(ChallengeTreeError::NotCrateOrSyn(syn_impl_item_index));
        }
        if self.options.verbose() {
            println!(
                "Adding implemented by link from '{}' to '{}'.",
                self.get_verbose_name_of_tree_node(source)?,
                self.get_verbose_name_of_tree_node(syn_impl_item_index)?
            );
        }
        self.tree
            .add_edge(source, syn_impl_item_index, EdgeType::Implementation);
        Ok(())
    }

    pub(crate) fn add_required_by_challenge_link(
        &mut self,
        source: NodeIndex,
        target: NodeIndex,
    ) -> TreeResult<()> {
        if !self.is_source_item(source) {
            return Err(ChallengeTreeError::NotCrateOrSyn(source));
        }
        if !self.is_source_item(target) {
            return Err(ChallengeTreeError::NotCrateOrSyn(target));
        }
        if self.options.verbose() {
            let source_module = if !self.is_crate_or_module(source) {
                self.get_syn_module_index(source)
            } else {
                None
            };
            let target_module = if !self.is_crate_or_module(source) {
                self.get_syn_module_index(target)
            } else {
                None
            };
            if source_module.is_some() && source_module == target_module {
                println!(
                    "Adding required by challenge link from '{}' to '{}' inside '{}'.",
                    self.get_verbose_name_of_tree_node(source)?,
                    self.get_verbose_name_of_tree_node(target)?,
                    self.get_verbose_name_of_tree_node(source_module.unwrap())?
                );
            } else {
                let source_string = if let Some(sm) = source_module {
                    format!(
                        "{}::{}",
                        self.get_verbose_name_of_tree_node(sm)?,
                        self.get_verbose_name_of_tree_node(source)?
                    )
                } else {
                    self.get_verbose_name_of_tree_node(source)?
                };
                let target_string = if let Some(tm) = target_module {
                    format!(
                        "{}::{}",
                        self.get_verbose_name_of_tree_node(tm)?,
                        self.get_verbose_name_of_tree_node(target)?
                    )
                } else {
                    self.get_verbose_name_of_tree_node(target)?
                };
                println!(
                    "Adding required by challenge link from '{}' to '{}'.",
                    source_string, target_string
                );
            }
        }
        self.tree
            .add_edge(source, target, EdgeType::RequiredByChallenge);
        Ok(())
    }

    pub(crate) fn add_challenge_links_for_referenced_nodes_of_item(
        &mut self,
        item_to_check: NodeIndex,
        seen_check_items: &mut HashSet<NodeIndex>,
    ) -> TreeResult<()> {
        if seen_check_items.insert(item_to_check) {
            let mut challenge_collector = SynReferenceMapper::new(self, item_to_check);
            match self.tree.node_weight(item_to_check) {
                Some(NodeType::SynItem(Item::Use(_))) => {
                    challenge_collector.reference_use_tree_nodes()?
                }
                Some(NodeType::SynItem(Item::Impl(item_impl))) => {
                    // with challenge collector trait and impl item will be referenced
                    challenge_collector.visit_item_impl(item_impl);
                    if item_impl.trait_.is_some() {
                        for (impl_item_index, _) in self.iter_syn_impl_item(item_to_check) {
                            challenge_collector.add_reference_node(impl_item_index);
                        }
                    }
                }
                Some(NodeType::SynItem(Item::Trait(trait_impl))) => {
                    challenge_collector.visit_item_trait(trait_impl);
                    for (trait_item_index, _) in self.iter_syn_trait_item(item_to_check) {
                        challenge_collector.add_reference_node(trait_item_index);
                    }
                }
                Some(NodeType::SynItem(item)) => {
                    challenge_collector.visit_item(item);
                }
                Some(NodeType::SynImplItem(impl_item)) => {
                    challenge_collector.visit_impl_item(impl_item);
                    if let Some(impl_block_index) =
                        self.get_parent_index_by_edge_type(item_to_check, EdgeType::Syn)
                    {
                        // reference impl block of required impl items
                        challenge_collector.add_reference_node(impl_block_index);
                    }
                }
                Some(NodeType::SynTraitItem(trait_item)) => {
                    challenge_collector.visit_trait_item(trait_item);
                }
                _ => return Ok(()),
            }
            // check collected node references
            for node_reference in challenge_collector.referenced_nodes.iter() {
                if seen_check_items.contains(node_reference) {
                    continue;
                }
                self.add_required_by_challenge_link(item_to_check, *node_reference)?;
                self.add_challenge_links_for_referenced_nodes_of_item(
                    *node_reference,
                    seen_check_items,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn add_lib_dependency_as_mod_to_fusion(
        &mut self,
        lib_crate_index: NodeIndex,
        fusion_node_index: NodeIndex,
    ) -> TreeResult<()> {
        let Some(NodeType::LibCrate(src_file)) = self.tree.node_weight(lib_crate_index) else {
            return Err(
                anyhow!("{}", add_context!("Expected required lib crate src file.")).into(),
            );
        };
        let new_mod = ItemMod {
            // only keep cfg attributes
            attrs: src_file
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("cfg"))
                .map(|a| a.to_owned())
                .collect(),

            vis: Visibility::Public(token::Pub::default()),
            unsafety: None,
            mod_token: token::Mod::default(),
            ident: Ident::new(&src_file.name, Span::call_site()),
            content: Some((token::Brace::default(), vec![])),
            semi: None,
        };
        let fusion_mod_index = self.tree.add_node(NodeType::SynItem(Item::Mod(new_mod)));
        self.node_mapping.insert(lib_crate_index, fusion_mod_index);
        // add new fusion mod index to content order of fusion
        let Some(fusion_item_order) = self.item_order.get_mut(&fusion_node_index) else {
            return Err(anyhow!(add_context!("Expected challenge item order.")).into());
        };
        fusion_item_order.push(fusion_mod_index);
        self.tree
            .add_edge(fusion_node_index, fusion_mod_index, EdgeType::Syn);
        if self.options.verbose() {
            println!(
                "Fusing '{}' to tree.",
                self.get_verbose_name_of_tree_node(fusion_mod_index)?
            );
        }
        // now add content of crate to new mod
        self.add_required_mod_content_to_fusion(lib_crate_index, fusion_mod_index)?;
        Ok(())
    }

    pub(crate) fn add_required_mod_content_to_fusion(
        &mut self,
        mod_index: NodeIndex,
        fusion_mod_index: NodeIndex,
    ) -> TreeResult<()> {
        let mod_content: Vec<(NodeIndex, Item)> = self
            .iter_syn_item_neighbors(mod_index)
            .filter(|(n, _)| self.is_required_by_challenge(*n))
            .map(|(n, i)| (n, i.to_owned()))
            .collect();
        let mut sub_mods: Vec<(NodeIndex, NodeIndex)> = Vec::new();
        for (item_index, item) in mod_content {
            let new_fusion_item_index = match item {
                Item::Mod(_) => {
                    let new_mod_index = self.tree.add_node(NodeType::SynItem(item));
                    sub_mods.push((item_index, new_mod_index));
                    new_mod_index
                }
                Item::Use(mut item_use) => {
                    let new_item_use = match self.get_path_root(item_index, (&item_use).into())? {
                        PathElement::Item(path_root) => {
                            if self.is_crate(path_root) && !item_use.tree.path_root_is_keyword() {
                                let new_use_root = UsePath {
                                    ident: Ident::new("crate", Span::call_site()),
                                    colon2_token: token::PathSep::default(),
                                    tree: Box::new(item_use.tree.to_owned()),
                                };
                                item_use.tree = UseTree::Path(new_use_root);
                                item_use
                            } else {
                                item_use
                            }
                        }
                        _ => item_use,
                    };
                    self.tree
                        .add_node(NodeType::SynItem(Item::Use(new_item_use)))
                }
                Item::Impl(item_impl) => {
                    let mut new_impl_item = item_impl;
                    let ordered_required_impl_items: Vec<ImplItem> = self.item_order[&item_index]
                        .iter()
                        .filter_map(|on| {
                            self.iter_syn_impl_item(item_index)
                                .filter(|(n, _)| {
                                    new_impl_item.trait_.is_some()
                                        || self.is_required_by_challenge(*n)
                                })
                                .find(|(rn, _)| on == rn)
                                .map(|(_, ri)| ri.to_owned())
                        })
                        .collect();
                    if ordered_required_impl_items.is_empty() {
                        // None if the impl items are required by challenge. Therefore do not add impl item to fusion
                        continue;
                    }
                    new_impl_item.items = ordered_required_impl_items;
                    // fold crate keyword to all path statements in new_impl_item, which path roots are crates
                    let mut path_folder = FusedDepPathFolder {
                        graph: self,
                        node: item_index,
                    };
                    let new_impl_item = path_folder.fold_item_impl(new_impl_item);
                    self.tree
                        .add_node(NodeType::SynItem(Item::Impl(new_impl_item)))
                }
                Item::Trait(trait_impl) => {
                    let mut new_trait_item = trait_impl;
                    let ordered_required_trait_items: Vec<TraitItem> = self.item_order[&item_index]
                        .iter()
                        .filter_map(|on| {
                            self.iter_syn_trait_item(item_index)
                                .find(|(rn, _)| on == rn)
                                .map(|(_, ri)| ri.to_owned())
                        })
                        .collect();
                    new_trait_item.items = ordered_required_trait_items;
                    // fold crate keyword to all path statements in new_trait_item, which path roots are crates
                    let mut path_folder = FusedDepPathFolder {
                        graph: self,
                        node: item_index,
                    };
                    let new_trait_item = path_folder.fold_item_trait(new_trait_item);
                    self.tree
                        .add_node(NodeType::SynItem(Item::Trait(new_trait_item)))
                }
                _ => {
                    // fold crate keyword to all path statements in item, which path roots are crates
                    let mut path_folder = FusedDepPathFolder {
                        graph: self,
                        node: item_index,
                    };
                    let item = path_folder.fold_item(item);
                    self.tree.add_node(NodeType::SynItem(item))
                }
            };
            self.node_mapping.insert(item_index, new_fusion_item_index);
            self.tree
                .add_edge(fusion_mod_index, new_fusion_item_index, EdgeType::Syn);
            if self.options.verbose() {
                println!(
                    "Fusing '{}' to tree.",
                    self.get_verbose_name_of_tree_node(new_fusion_item_index)?
                );
            }
        }
        // add item order of fusion mod
        let item_order = self
            .item_order
            .get(&mod_index)
            .context(add_context!("Expected item order of mod."))?;
        self.item_order.insert(
            fusion_mod_index,
            item_order
                .iter()
                .filter_map(|n| self.node_mapping.get(n).cloned())
                .collect(),
        );
        // add sub mods to tree
        for (sub_mod_index, sub_mod_fusion_index) in sub_mods {
            self.add_required_mod_content_to_fusion(sub_mod_index, sub_mod_fusion_index)?;
        }

        Ok(())
    }

    pub(crate) fn add_fusion_bin_crate(&mut self) -> TreeResult<NodeIndex> {
        let path = self.get_fusion_file_path()?;
        let fusion_bin_dir = path
            .parent()
            .context(add_context!("Expected parent of fusion file path."))?;
        fs::create_dir_all(fusion_bin_dir)?;
        let fusion_file_name = path
            .file_stem()
            .context(add_context!("Expected file stem of fusion file path."))?
            .to_owned();
        let (_, challenge_src_file) = self
            .get_challenge_bin_crate()
            .context(add_context!("Expected challenge bin crate."))?;
        let fusion_src_file = SrcFile {
            name: fusion_file_name,
            path: path.to_owned(),
            shebang: challenge_src_file.shebang.clone(),
            attrs: challenge_src_file.attrs.clone(),
        };
        let fusion_node_index = self.tree.add_node(NodeType::BinCrate(fusion_src_file));
        // challenge package is at index 0
        self.tree
            .add_edge(0.into(), fusion_node_index, EdgeType::Crate);
        if self.options.verbose() {
            println!(
                "Fusing '{}' at path '{}' to tree.",
                self.get_verbose_name_of_tree_node(fusion_node_index)?,
                path
            );
        }
        Ok(fusion_node_index)
    }
}

impl<O, S> CgData<O, S> {
    pub(crate) fn resolving_relative_source_path(
        &self,
        path_item_index: NodeIndex,
        source_path: SourcePath,
    ) -> TreeResult<SourcePath> {
        // get path properties
        let (segments, glob, rename) = match &source_path {
            SourcePath::Group => {
                unreachable!("use groups have been expanded before.");
            }
            // glob is still possible, if it points to external crate
            SourcePath::Glob(segments) => (segments, true, None),
            SourcePath::Name(segments) => (segments, false, None),
            SourcePath::Rename(segments, renamed) => (segments, false, Some(renamed.to_owned())),
        };
        let mut remaining_external_segments: Option<Vec<Ident>> = None;
        let mut path_leaf: Option<NodeIndex> = None;
        let mut path_walker = SourcePathWalker::new(source_path.clone(), path_item_index);
        while let Some(path_element) = path_walker.next(self) {
            match path_element {
                PathElement::PathCouldNotBeParsed => return Ok(source_path),
                PathElement::Group => {
                    unreachable!("Use groups have been expanded before.");
                }
                PathElement::Glob(_) => {
                    unreachable!(
                        "Local use globs have been expanded before. Only external globs are possible, which will return \
                         PathElement::ExternalPackage before reaching glob."
                    );
                }
                PathElement::ExternalItem(_) | PathElement::ExternalGlob(_) => {
                    if let Some(leaf_index) = path_leaf {
                        // This is only possible, if a path element points toward a use statement,
                        // which imports external code. Minimize path to this use statement and
                        // append remaining segments of external use statement
                        if let Some(external_use_ident) = self.get_ident(leaf_index) {
                            if let Some(pos) =
                                segments.iter().position(|s| *s == external_use_ident)
                            {
                                remaining_external_segments = Some(Vec::from(&segments[pos + 1..]));
                                break;
                            }
                        }
                        return Ok(source_path);
                    } else {
                        // path directly starts with external package
                        return Ok(source_path);
                    }
                }
                PathElement::Item(item_index) | PathElement::ItemRenamed(item_index, _) => {
                    // collect item index, rename is already extracted from SourcePath
                    path_leaf = Some(item_index);
                    //segments_slice = &segments_slice[1..];
                }
            }
        }
        // compare crates of active path leaf and path_item_index
        let path_leaf = path_leaf.context(add_context!("Expected index of path leaf."))?;
        let path_leaf_nodes = self.get_crate_path_nodes(path_leaf);
        let path_item_nodes = self.get_crate_path_nodes(path_item_index);
        let mut new_path: Vec<Ident> = if path_leaf_nodes[0] != path_item_nodes[0] {
            // return path of leaf starting from it's crate
            path_leaf_nodes
                .iter()
                .map(|n| {
                    self.get_ident(*n)
                        .ok_or(anyhow!("{}", add_context!("Expected ident of path node.")).into())
                })
                .collect::<TreeResult<Vec<_>>>()?
        } else {
            // identify best path inside crate from path_item_index to path_leaf
            let pos_junction = path_item_nodes
                .iter()
                .zip(path_leaf_nodes.iter())
                .take_while(|(a, b)| a == b)
                .count();
            let (from_junction_leaf_ident, num_super) = if pos_junction == path_leaf_nodes.len() {
                // path_item is at same level or deeper in tree than path_leaf
                let leaf_ident = self
                    .get_ident(path_leaf_nodes[pos_junction - 1])
                    .context(add_context!("Expected ident of path node."))?;
                let num_super = path_item_nodes.len() - pos_junction;
                (vec![leaf_ident], num_super)
            } else {
                // path leaf is deeper in tree than path_item
                let from_junction_leaf_ident = path_leaf_nodes[pos_junction..]
                    .iter()
                    .map(|n| {
                        self.get_ident(*n).ok_or(
                            anyhow!("{}", add_context!("Expected ident of path node.")).into(),
                        )
                    })
                    .collect::<TreeResult<Vec<_>>>()?;
                let num_super = path_item_nodes.len() - pos_junction - 1;
                (from_junction_leaf_ident, num_super)
            };
            let mut new_path = vec![Ident::new("super", Span::call_site()); num_super];
            new_path.extend(from_junction_leaf_ident);
            new_path
        };

        if let Some(res) = remaining_external_segments.take() {
            new_path.extend(res);
        }

        let new_path = match (glob, rename) {
            (true, None) => SourcePath::Glob(new_path),
            (false, Some(renamed)) => SourcePath::Rename(new_path, renamed),
            (false, None) => SourcePath::Name(new_path),
            _ => unreachable!(),
        };
        Ok(new_path)
    }

    pub(crate) fn update_required_mod_content(&mut self, mod_index: NodeIndex) -> TreeResult<()> {
        // recursive tree traversal to mod without further mods
        let item_mod_indices: Vec<NodeIndex> = self
            .iter_syn_item_neighbors(mod_index)
            .filter_map(|(n, i)| match i {
                Item::Mod(_) => Some(n),
                _ => None,
            })
            .collect();
        for item_mod_index in item_mod_indices {
            self.update_required_mod_content(item_mod_index)?;
        }

        if self.is_crate(mod_index) {
            // end of recursive updating
            return Ok(());
        }

        // get sorted list of mod items
        let mod_content: Vec<Item> = self.get_sorted_mod_content(mod_index)?;
        // update current mod
        if let Some(NodeType::SynItem(Item::Mod(item_mod))) = self.tree.node_weight_mut(mod_index) {
            item_mod.content = Some((token::Brace::default(), mod_content));
            item_mod.semi = None;
        }
        Ok(())
    }
}

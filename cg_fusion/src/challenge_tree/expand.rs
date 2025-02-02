// functions to expand the challenge tree
use super::{
    ChallengeTreeError, EdgeType, LocalPackage, NodeType, SrcFile, SynReferenceMapper, TreeResult,
};
use crate::{
    add_context,
    challenge_tree::PathElement,
    configuration::CgCli,
    parsing::{load_syntax, ItemName, UseTreeExtras},
    CgData,
};

use anyhow::{anyhow, Context};
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::stable_graph::NodeIndex;
use proc_macro2::Span;
use quote::ToTokens;
use std::collections::{HashMap, HashSet};
use std::fs;
use syn::{token, visit::Visit, Ident, Item, ItemMod, UsePath, UseTree, Visibility};

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
        if let Some(target) = self
            .get_local_package(package_node_index)?
            .metadata
            .get_library_target_of_root_package()?
        {
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
        } else {
            Ok(None)
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
                        println!("Adding '{}' (Use) to tree.", item_use.to_token_stream());
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
        let (items, mod_src_file) = if let Some(NodeType::SynItem(Item::Mod(item_mod))) =
            self.tree.node_weight_mut(item_mod_index)
        {
            let mod_data = if let Some(mod_content) = item_mod.content.take() {
                // with take() mod_content of item_mod is set to None
                if self.options.verbose() {
                    println!(
                        "Adding inline '{}' to tree.",
                        self.get_verbose_name_of_tree_node(item_mod_index)?
                    );
                }
                (mod_content.1, None)
            } else {
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
            };
            mod_data
        } else {
            return Err(anyhow!(add_context!("Expecting item mod.")).into());
        };

        let mut item_order: Vec<NodeIndex> = Vec::new();
        if let Some((src_file, mod_dir)) = mod_src_file {
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
        } else {
            for content_item in items.iter() {
                item_order.push(self.add_syn_item(content_item, dir_path, item_mod_index)?);
            }
        }
        self.item_order.insert(item_mod_index, item_order);

        Ok(())
    }

    fn add_syn_item_impl(&mut self, item_impl_index: NodeIndex) -> TreeResult<()> {
        let items = if let Some(NodeType::SynItem(Item::Impl(item_impl))) =
            self.tree.node_weight_mut(item_impl_index)
        {
            // mem::take takes all items from item_impl.items, leaving it empty
            std::mem::take(&mut item_impl.items)
        } else {
            return Err(anyhow!(add_context!("Expected impl item.")).into());
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
        let items = if let Some(NodeType::SynItem(Item::Trait(trait_impl))) =
            self.tree.node_weight_mut(item_trait_index)
        {
            // mem::take takes all items from item_impl.items, leaving it empty
            std::mem::take(&mut trait_impl.items)
        } else {
            return Err(anyhow!(add_context!("Expected trait item.")).into());
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
                    for (impl_block_index, _) in self.iter_impl_blocks_of_item(item_to_check) {
                        challenge_collector.add_reference_node(impl_block_index);
                    }
                }
                Some(NodeType::SynImplItem(impl_item)) => {
                    challenge_collector.visit_impl_item(impl_item)
                }
                Some(NodeType::SynTraitItem(trait_item)) => {
                    challenge_collector.visit_trait_item(trait_item)
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
        if self.options.verbose() {
            println!("A")
        }
        let fusion_mod_index = self.tree.add_node(NodeType::SynItem(Item::Mod(new_mod)));
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
        let mut node_mapping: HashMap<NodeIndex, NodeIndex> = HashMap::new();
        let mut sub_mods: Vec<(NodeIndex, NodeIndex)> = Vec::new();
        for (item_index, item) in mod_content {
            let new_fusion_item_index = match item {
                Item::Mod(_) => {
                    let new_mod_index = self.tree.add_node(NodeType::SynItem(item));
                    sub_mods.push((item_index, new_mod_index));
                    new_mod_index
                }
                Item::Use(mut item_use) => {
                    let new_item_use = if let PathElement::Item(path_root) =
                        self.get_path_root(item_index, (&item_use).into())?
                    {
                        if self.is_crate(path_root)
                            && !item_use.tree.is_use_tree_root_path_keyword()
                        {
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
                    } else {
                        item_use
                    };
                    self.tree
                        .add_node(NodeType::SynItem(Item::Use(new_item_use)))
                }
                Item::Impl(mut item_impl) => {
                    let new_item_impl = if item_impl.trait_.is_none() {
                        let required_impl_item_names: Vec<Ident> = self
                            .iter_syn_impl_item(item_index)
                            .filter_map(|(n, i)| {
                                self.is_required_by_challenge(n).then_some(i.to_owned())
                            })
                            .filter_map(|i| ItemName::from(&i).get_ident_in_name_space())
                            .collect();
                        // keep original order by only retaining required impl items
                        item_impl.items.retain(|i| {
                            if let Some(name) = ItemName::from(i).get_ident_in_name_space() {
                                required_impl_item_names.contains(&name)
                            } else {
                                false
                            }
                        });
                        item_impl
                    } else {
                        item_impl
                    };
                    self.tree
                        .add_node(NodeType::SynItem(Item::Impl(new_item_impl)))
                }
                _ => self.tree.add_node(NodeType::SynItem(item)),
            };
            node_mapping.insert(item_index, new_fusion_item_index);
            self.tree
                .add_edge(fusion_mod_index, new_fusion_item_index, EdgeType::Syn);
            if self.options.verbose() {
                println!(
                    "Fusing '{}' to tree.",
                    self.get_verbose_name_of_tree_node(new_fusion_item_index)?
                );
            }
        }
        // add mod intern implementation links
        let mod_intern_impl_links: Vec<(NodeIndex, NodeIndex)> = self
            .iter_syn_item_neighbors(mod_index)
            .flat_map(|(n, _)| self.iter_impl_blocks_of_item(n).map(move |(ni, _)| (n, ni)))
            .filter(|(n, ni)| node_mapping.contains_key(n) && node_mapping.contains_key(ni))
            .collect();
        for (item_index, impl_block_index) in mod_intern_impl_links {
            let new_fusion_item_index = node_mapping.get(&item_index).unwrap();
            let new_fusion_impl_block_index = node_mapping.get(&impl_block_index).unwrap();
            self.tree.add_edge(
                *new_fusion_item_index,
                *new_fusion_impl_block_index,
                EdgeType::Implementation,
            );
            if self.options.verbose() {
                println!(
                    "Fusing implementation link from '{}' to '{}'.",
                    self.get_verbose_name_of_tree_node(*new_fusion_item_index)?,
                    self.get_verbose_name_of_tree_node(*new_fusion_impl_block_index)?
                );
            }
        }
        // add sub mods to tree
        for (sub_mod_index, sub_mod_fusion_index) in sub_mods {
            self.add_required_mod_content_to_fusion(sub_mod_index, sub_mod_fusion_index)?;
        }

        Ok(())
    }
}

impl<O: CgCli, S> CgData<O, S> {
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

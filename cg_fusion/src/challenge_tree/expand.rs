// functions to expand the challenge tree
use super::{
    ChallengeTreeError, EdgeType, LocalPackage, NodeType, SrcFile, SynReferenceMapper, TreeResult,
};
use crate::{
    add_context,
    configuration::CgCli,
    parsing::{load_syntax, ItemName},
    CgData,
};

use anyhow::{anyhow, Context};
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::stable_graph::NodeIndex;
use quote::ToTokens;
use std::collections::HashSet;
use std::fs;
use syn::{visit::Visit, Item, ItemImpl, ItemMod, ItemTrait};

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
            Item::Mod(item_mod) => {
                self.add_syn_item_mod(item_mod, dir_path, item_index)?;
            }
            // if item is impl, add content of impl to tree
            Item::Impl(item_impl) => {
                if self.options.verbose() {
                    println!("Adding '{}' to tree.", ItemName::from(item));
                }
                self.add_syn_item_impl(item_impl, item_index)?;
            }
            // if item is impl, add content of impl to tree
            Item::Trait(item_trait) => {
                if self.options.verbose() {
                    println!("Adding '{}' to tree.", ItemName::from(item));
                }
                self.add_syn_item_trait(item_trait, item_index)?;
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
        item_mod: &ItemMod,
        dir_path: &Utf8PathBuf,
        item_mod_index: NodeIndex,
    ) -> TreeResult<()> {
        match item_mod.content {
            Some((_, ref content)) => {
                if self.options.verbose() {
                    println!(
                        "Adding inline '{}' to tree.",
                        self.get_verbose_name_of_tree_node(item_mod_index)?
                    );
                }
                for content_item in content.iter() {
                    self.add_syn_item(content_item, dir_path, item_mod_index)?;
                }
            }
            None => {
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
                // add src file of module to tree
                if self.options.verbose() {
                    println!(
                        "Adding module src file '{}' at path '{}' to tree.",
                        self.get_verbose_name_of_tree_node(item_mod_index)?,
                        path,
                    );
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
                let mod_node_index = self.tree.add_node(NodeType::Module(src_file));
                self.tree
                    .add_edge(mod_node_index, item_mod_index, EdgeType::Module);

                // add items of module src file to tree
                for content_item in mod_syntax.items.iter() {
                    self.add_syn_item(content_item, &mod_dir, item_mod_index)?;
                }
            }
        }
        Ok(())
    }

    fn add_syn_item_impl(
        &mut self,
        item_impl: &ItemImpl,
        item_impl_index: NodeIndex,
    ) -> TreeResult<()> {
        // Add impl items
        for impl_item in item_impl.items.iter() {
            if self.options.verbose() {
                println!("Adding '{}' to tree.", ItemName::from(impl_item));
            }
            let impl_item_index = self
                .tree
                .add_node(NodeType::SynImplItem(impl_item.to_owned()));
            self.tree
                .add_edge(item_impl_index, impl_item_index, EdgeType::Syn);
        }
        Ok(())
    }

    fn add_syn_item_trait(
        &mut self,
        item_trait: &ItemTrait,
        item_trait_index: NodeIndex,
    ) -> TreeResult<()> {
        // Add impl items
        for trait_item in item_trait.items.iter() {
            if self.options.verbose() {
                println!("Adding '{}' to tree.", ItemName::from(trait_item));
            }
            let trait_item_index = self
                .tree
                .add_node(NodeType::SynTraitItem(trait_item.to_owned()));
            self.tree
                .add_edge(item_trait_index, trait_item_index, EdgeType::Syn);
        }
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
            let mut challenge_collector = SynReferenceMapper::new(&self, item_to_check);
            match self.tree.node_weight(item_to_check) {
                Some(NodeType::SynItem(Item::Mod(_)))            // do not reference nodes in these items, since
                | Some(NodeType::SynItem(Item::Impl(_)))         // we will process their sub items if they
                | Some(NodeType::SynItem(Item::Trait(_))) => (), // are linked as required by challenge.
                Some(NodeType::SynItem(Item::Use(_))) => challenge_collector.reference_use_tree_nodes()?,
                Some(NodeType::SynItem(item)) => challenge_collector.visit_item(item),
                Some(NodeType::SynImplItem(impl_item)) => challenge_collector.visit_impl_item(impl_item),
                Some(NodeType::SynTraitItem(trait_item)) => {
                    challenge_collector.visit_trait_item(trait_item)
                }
                _ => return Ok(()),
            }
            // check collected node references
            for node_reference in challenge_collector.referenced_nodes.iter() {
                if self.is_syn_impl_item(*node_reference) || self.is_syn_trait_item(*node_reference)
                {
                    let impl_or_trait_index = self
                        .get_parent_index_by_edge_type(*node_reference, EdgeType::Syn)
                        .context(add_context!("Expected impl or trait item."))?;
                    if !self.is_required_by_challenge(impl_or_trait_index) {
                        self.add_required_by_challenge_link(item_to_check, impl_or_trait_index)?;
                    }
                }
                self.add_required_by_challenge_link(item_to_check, *node_reference)?;
                self.add_challenge_links_for_referenced_nodes_of_item(*node_reference, seen_check_items)?;
            }
        }
        Ok(())
    }
}

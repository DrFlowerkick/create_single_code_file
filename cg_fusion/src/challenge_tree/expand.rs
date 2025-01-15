// functions to expand the challenge tree
use super::{ChallengeTreeError, CrateFile, EdgeType, LocalPackage, NodeType, TreeResult};
use crate::{
    add_context,
    configuration::CgCli,
    parsing::{load_syntax, ItemName},
    CgData,
};

use anyhow::anyhow;
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::stable_graph::NodeIndex;
use quote::ToTokens;
use syn::{token::Brace, Item, ItemImpl, ItemMod, ItemTrait};

impl<O: CgCli, S> CgData<O, S> {
    pub fn add_local_package(&mut self, source: NodeIndex, package: LocalPackage) -> NodeIndex {
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

    pub fn link_to_package(&mut self, source: NodeIndex, target: NodeIndex) {
        self.tree.add_edge(source, target, EdgeType::Dependency);
    }

    pub fn add_external_supported_package(
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

    pub fn add_external_unsupported_package(
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

    pub fn add_binary_crate_to_package(
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

        // get syntax of src file
        let syntax = load_syntax(&path)?;
        // generate node value
        let crate_file = CrateFile {
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

    pub fn add_library_crate_to_package(
        &mut self,
        package_node_index: NodeIndex,
    ) -> TreeResult<Option<NodeIndex>> {
        // get bin path from metadata
        if let Some(target) = self
            .get_local_package(package_node_index)?
            .metadata
            .get_library_target_of_root_package()?
        {
            // get syntax of src file
            let syntax = load_syntax(&target.src_path)?;
            // generate node value
            let crate_file = CrateFile {
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

    pub fn add_syn_item(
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
                if self.options.verbose() {
                    println!(
                        "Adding '{}' at path '{}' to tree.",
                        self.get_verbose_name_of_tree_node(item_mod_index)?,
                        path
                    );
                }
                // get syntax of src file
                let mod_syntax = load_syntax(&path)?;
                for content_item in mod_syntax.items.iter() {
                    self.add_syn_item(content_item, &mod_dir, item_mod_index)?;
                }
                // change mod item in tree to inline module
                // ToDo: do we need to do this at this state of execution? Or should we do this during merging?
                let mut inline_mod = item_mod.to_owned();
                let inline_items: Vec<Item> = self
                    .iter_syn_item_neighbors(item_mod_index)
                    .map(|(_, i)| i.to_owned())
                    .collect();
                inline_mod.content = Some((Brace::default(), inline_items));
                if let Some(node_weight) = self.tree.node_weight_mut(item_mod_index) {
                    *node_weight = NodeType::SynItem(Item::Mod(inline_mod));
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

    pub fn add_implementation_link(
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

    pub fn add_required_by_challenge_link(
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
}

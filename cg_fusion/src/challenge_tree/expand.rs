// functions to expand the challenge tree

use super::{ChallengeTreeError, CrateFile, EdgeType, LocalPackage, NodeTyp, TreeResult};
use crate::{
    add_context,
    configuration::CliInput,
    parsing::{load_syntax, ItemName},
    CgData,
};

use anyhow::anyhow;
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::stable_graph::NodeIndex;
use quote::ToTokens;
use syn::{token::Brace, Item, ItemImpl, ItemMod, Type};

impl<O: CliInput, S> CgData<O, S> {
    pub fn add_local_package(&mut self, source: NodeIndex, package: LocalPackage) -> NodeIndex {
        if self.options.verbose() {
            println!(
                "Found local dependency '{}' at '{}'",
                package.name, package.path
            );
        }
        let package_index = self.tree.add_node(NodeTyp::LocalPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
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
        if self.options.verbose() {
            println!("Found external supported dependency '{}'", package);
        }
        let package_index = self
            .tree
            .add_node(NodeTyp::ExternalSupportedPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
        package_index
    }

    pub fn add_external_unsupported_package(
        &mut self,
        source: NodeIndex,
        package: String,
    ) -> NodeIndex {
        if self.options.verbose() {
            println!("Found external unsupported dependency '{}'", package);
        }
        let package_index = self
            .tree
            .add_node(NodeTyp::ExternalUnsupportedPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
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
            path,
            shebang: syntax.shebang,
            attrs: syntax.attrs,
        };

        if self.options.verbose() {
            println!(
                "Adding binary crate '{}' with path '{}' to tree...",
                crate_file.name, crate_file.path
            );
        }

        let crate_node_index = self.tree.add_node(NodeTyp::BinCrate(crate_file));
        self.tree
            .add_edge(package_node_index, crate_node_index, EdgeType::Crate);

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

            if self.options.verbose() {
                println!(
                    "Adding library crate '{}' with path '{}' to tree...",
                    crate_file.name, crate_file.path
                );
            }

            let crate_node_index = self.tree.add_node(NodeTyp::LibCrate(crate_file));
            self.tree
                .add_edge(package_node_index, crate_node_index, EdgeType::Crate);

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
        let item_index: NodeIndex = self.tree.add_node(NodeTyp::SynItem(item.to_owned()));
        self.tree.add_edge(source_index, item_index, EdgeType::Syn);

        match item {
            // if item is module, add content of module to tree
            Item::Mod(item_mod) => {
                self.add_syn_item_mod(item_mod, dir_path, item_index)?;
            }
            // if item is impl, but not impl trait, add content of impl to tree
            Item::Impl(item_impl) => {
                self.add_syn_item_impl(item_impl, item_index)?;
            }
            // if item is use statement, at this state of tree a unique name cannot be guaranteed.
            // therefore just print use statement if verbose option
            Item::Use(item_use) => {
                if self.options.verbose() {
                    println!("Adding syn item '{}' to tree.", item_use.to_token_stream());
                }
            }
            _ => {
                if self.options.verbose() {
                    println!("Adding syn item '{}' to tree.", ItemName::from(item));
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
        let module = item_mod.ident.to_string();
        match item_mod.content {
            Some((_, ref content)) => {
                if self.options.verbose() {
                    println!("found inline module '{}', adding it to tree...", module);
                }
                for content_item in content.iter() {
                    self.add_syn_item(content_item, dir_path, item_mod_index)?;
                }
            }
            None => {
                // set module directory
                let mod_dir = dir_path.join(module.as_str());
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
                    println!("found module '{}' at '{}', adding to tree...", module, path);
                }
                // get syntax of src file
                let mod_syntax = load_syntax(&path)?;
                for content_item in mod_syntax.items.iter() {
                    self.add_syn_item(content_item, &mod_dir, item_mod_index)?;
                }
                // change mod item in tree to inline module
                let mut inline_mod = item_mod.to_owned();
                let inline_items: Vec<Item> = self
                    .iter_syn_neighbors(item_mod_index)
                    .map(|(_, i)| i.to_owned())
                    .collect();
                inline_mod.content = Some((Brace::default(), inline_items));
                if let Some(node_weight) = self.tree.node_weight_mut(item_mod_index) {
                    *node_weight = NodeTyp::SynItem(Item::Mod(inline_mod));
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
        if let Some((_, ref impl_trait_path, _)) = item_impl.trait_ {
            if self.options.verbose() {
                if let Type::Path(type_path) = item_impl.self_ty.as_ref() {
                    println!(
                        "Adding syn impl block item of '{}' for trait '{}'.",
                        type_path.path.segments.to_token_stream(),
                        impl_trait_path.segments.to_token_stream(),
                    );
                }
            }
            // trait impl is not expanded, since all trait items must be implemented
            return Ok(());
        }
        if let Type::Path(type_path) = item_impl.self_ty.as_ref() {
            println!(
                "Adding syn impl block item of '{}'.",
                type_path.path.segments.to_token_stream(),
            );
        }
        for impl_item in item_impl.items.iter() {
            if self.options.verbose() {
                println!(
                    "Adding syn impl item '{}' to tree.",
                    ItemName::from(impl_item)
                );
            }
            let impl_item_index = self
                .tree
                .add_node(NodeTyp::SynImplItem(impl_item.to_owned()));
            self.tree
                .add_edge(item_impl_index, impl_item_index, EdgeType::Syn);
        }
        Ok(())
    }

    pub fn add_usage_link(&mut self, source: NodeIndex, target: NodeIndex) -> TreeResult<()> {
        // test for existing nodes
        let source_syn = self
            .get_syn_item(source)
            .ok_or(ChallengeTreeError::NotCrateOrSyn(source))?;
        let target_name = match self.tree.node_weight(target) {
            Some(NodeTyp::SynItem(item)) => {
                format!("{}", ItemName::from(item))
            }
            Some(NodeTyp::LibCrate(crate_file)) => crate_file.name.to_owned(),
            _ => {
                return Err(ChallengeTreeError::NotCrateOrSyn(target));
            }
        };
        if self.options.verbose() {
            let source = ItemName::from(source_syn);
            println!("Adding usage link from '{source}' to '{target_name}'.");
        }
        self.tree.add_edge(source, target, EdgeType::Usage);
        Ok(())
    }

    pub fn add_implementation_by_link(
        &mut self,
        source: NodeIndex,
        syn_impl_item_index: NodeIndex,
    ) -> TreeResult<()> {
        // test for existing nodes
        let source_syn = self
            .get_syn_item(source)
            .ok_or(ChallengeTreeError::NotCrateOrSyn(source))?;
        let syn_impl_item = self
            .get_syn_item(syn_impl_item_index)
            .ok_or(ChallengeTreeError::NotCrateOrSyn(syn_impl_item_index))?;
        if self.options.verbose() {
            let source = ItemName::from(source_syn);
            let trait_name = ItemName::from(syn_impl_item);
            if trait_name.extract_name().is_none() {
                println!("Adding implemented by link for '{source}'.");
            } else {
                println!("Adding implemented by link of '{trait_name}' for '{source}'.");
            }
        }
        self.tree.add_edge(source, syn_impl_item_index, EdgeType::Implementation);
        Ok(())
    }

    pub fn add_semantic_link(&mut self, source: NodeIndex, target: NodeIndex) -> TreeResult<()> {
        self.tree
            .node_weight(source)
            .ok_or(ChallengeTreeError::NotCrateOrSyn(source))?;
        self.tree
            .node_weight(target)
            .ok_or(ChallengeTreeError::NotCrateOrSyn(target))?;
        self.tree.add_edge(source, target, EdgeType::Semantic);
        Ok(())
    }
}

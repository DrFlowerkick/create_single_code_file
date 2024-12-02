// functions to expand the challenge tree

use super::{CrateFile, EdgeType, LocalPackage, NodeTyp, TreeResult};
use crate::{add_context, configuration::CliInput, parsing::load_syntax, CgData};

use anyhow::anyhow;
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::graph::NodeIndex;
use syn::{token::Brace, Item};

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
        let crate_file = CrateFile { name, path, syntax };

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
                syntax,
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
        let item_index = self.tree.add_node(NodeTyp::SynItem(item.to_owned()));
        self.tree.add_edge(source_index, item_index, EdgeType::Syn);

        // if item is module, add content of module to tree
        if let Item::Mod(item_mod) = item {
            let module = item_mod.ident.to_string();
            match item_mod.content {
                Some((_, ref content)) => {
                    if self.options.verbose() {
                        println!("found inline module '{}', adding it to tree...", module);
                    }
                    for content_item in content.iter() {
                        self.add_syn_item(content_item, dir_path, item_index)?;
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
                        self.add_syn_item(content_item, &mod_dir, item_index)?;
                    }
                    // change mod item in tree to inline module
                    let mut inline_mod = item_mod.to_owned();
                    let inline_items: Vec<Item> = self
                        .iter_syn_neighbors(item_index)
                        .map(|(_, i)| i.to_owned())
                        .collect();
                    inline_mod.content = Some((Brace::default(), inline_items));
                    if let Some(node_weight) = self.tree.node_weight_mut(item_index) {
                        *node_weight = NodeTyp::SynItem(Item::Mod(inline_mod));
                    }
                }
            }
        }
        Ok(item_index)
    }
}

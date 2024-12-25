// Linking all items, which are required by challenge

use super::AnalyzeState;
use crate::{
    add_context, configuration::CliInput, error::CgResult, parsing::get_name_of_item, CgData,
};
use anyhow::{anyhow, Context};
use petgraph::graph::NodeIndex;
use syn::{Item, UseTree};

#[derive(Debug)]
struct ItemsCheckSemantic {
    item: Item,
    node: NodeIndex,
    use_target_node: Option<NodeIndex>,
}

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn link_challenge_semantic(&mut self) -> CgResult<()> {
        let (index, _) = self.get_challenge_bin_crate().unwrap();
        let (main_index, _) = self
            .iter_syn_neighbors(index)
            .filter_map(|(n, i)| match i {
                Item::Fn(fn_item) => {
                    if fn_item.sig.ident == "main" {
                        Some((n, fn_item))
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .next()
            .context(add_context!("Expected main fn of challenge bin crate."))?;
        self.add_semantic_link(index, main_index)?;
        // ToDo: write function to
        // 1. collect neighbors and try to link them as long as there are no more new links,
        // either because there are no more items to link or unlinked items are not used in module.
        // 2. if any modules respectively their content are linked (either directly or via a use
        // statement), call this function calls itself recursive with index node of module and crate
        // index. If module is crate lib, than both indices are identical.
        // Linking items is item specific. Remember to although link items in impl items, if they
        // are used by challenge. Do not link to items in Trait impl. Instead link to trait impl itself.
        // For this to work next step is to create nodes for ImplItems!
        let neighbors_to_check = self.collect_neighbors_to_check(index, index)?;
        for neighbor in neighbors_to_check.iter() {
            println!(
                "item: {:?}\nnode: {:?}\nuse_target_node: {:?}",
                neighbor.item, neighbor.node, neighbor.use_target_node
            );
        }
        Ok(())
    }

    fn collect_neighbors_to_check(
        &self,
        node: NodeIndex,
        crate_index: NodeIndex,
    ) -> CgResult<Vec<ItemsCheckSemantic>> {
        self.iter_syn_neighbors_without_semantic_link(node)
            .map(|(index, item)| match item {
                Item::Use(_) => {
                    self.get_use_item_target_node(index, crate_index)
                        .map(|use_target_node| ItemsCheckSemantic {
                            item: self.get_syn_item(use_target_node).unwrap().to_owned(),
                            node: index,
                            use_target_node: Some(use_target_node),
                        })
                }
                _ => Ok(ItemsCheckSemantic {
                    item: item.to_owned(),
                    node: index,
                    use_target_node: None,
                }),
            })
            .collect::<Result<Vec<_>, _>>()
    }

    // ToDo: combine this function with usage::get_module_node_index_of_glob_use in one navigate function in challenge_tree
    fn get_use_item_target_node(
        &self,
        use_item_node_index: NodeIndex,
        crate_index: NodeIndex,
    ) -> CgResult<NodeIndex> {
        let mut use_tree = &self
            .tree
            .node_weight(use_item_node_index)
            .context(add_context!("Expected syn item"))?
            .get_use_item_from_syn_item_node()
            .context(add_context!("Expected syn ItemUse."))?
            .tree;
        let mut module_index = self
            .get_syn_item_module_index(use_item_node_index)
            .context(add_context!("Expected source index of syn item."))?;
        // walk trough the use path
        loop {
            match use_tree {
                UseTree::Path(use_path) => {
                    let module = use_path.ident.to_string();
                    match module.as_str() {
                        "crate" => {
                            // module is current crate
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
                            // some module, could be
                            // 1. sub module of current module
                            // 2. reimported module in current module
                            // 3. local package dependency
                            // 4. reimported external module (e.g. std::collections or rand::prelude)
                            if let Some((sub_module_index, _)) = self
                                .iter_syn_neighbors(module_index)
                                .filter_map(|(n, i)| match i {
                                    // 1. sub module of current module
                                    Item::Mod(item_mod) => Some((n, &item_mod.ident)),
                                    // 2. reimported module in current module
                                    // ToDo: how to handle re-imports?
                                    /*Item::Use(item_use) => {
                                        let item_import_index
                                    }*/
                                    _ => None,
                                })
                                .find(|(_, m)| **m == module)
                            {
                                module_index = sub_module_index;
                            } else if let Some((lib_crate_index, _)) =
                                self.iter_lib_crates().find(|(_, cf)| cf.name == module)
                            {
                                module_index = lib_crate_index;
                            } else {
                                Err(anyhow!(add_context!(format!(
                                    "Could not identify {}",
                                    module,
                                ))))?;
                            }
                        }
                    }
                    use_tree = &use_path.tree;
                }
                UseTree::Group(_) => {
                    Err(anyhow!(add_context!("Expected expanded use group.")))?;
                }
                UseTree::Glob(_) => {
                    Err(anyhow!(add_context!("Expected expanded use globs.")))?;
                }
                UseTree::Name(use_name) => {
                    // search for syn neighbors of module index with name of use_name
                    let (use_name_index, _) = self
                        .iter_syn_neighbors(module_index)
                        .filter_map(|(n, i)| {
                            get_name_of_item(i).extract_ident().map(|ident| (n, ident))
                        })
                        .find(|(_, name)| *name == use_name.ident)
                        .context(add_context!(format!(
                            "Expected {:?} at child of syn node {:?}.",
                            use_name.ident, module_index
                        )))?;
                    return Ok(use_name_index);
                }
                UseTree::Rename(use_rename) => {
                    // search for syn neighbors of module index with name of use_rename
                    let (use_name_index, _) = self
                        .iter_syn_neighbors(module_index)
                        .filter_map(|(n, i)| {
                            get_name_of_item(i).extract_ident().map(|ident| (n, ident))
                        })
                        .find(|(_, name)| *name == use_rename.rename)
                        .context(add_context!(format!(
                            "Expected {:?} at child of syn node {:?}.",
                            use_rename.rename, module_index
                        )))?;
                    return Ok(use_name_index);
                }
            }
        }
    }
}

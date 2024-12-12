// Linking all items, which are required by challenge

use super::AnalyzeState;
use crate::{
    add_context, configuration::CliInput, error::CgResult, parsing::get_name_of_item, CgData,
};
use anyhow::{anyhow, Context};
use petgraph::graph::NodeIndex;
use syn::{Item, UseTree};

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
                    if fn_item.sig.ident.to_string() == "main" {
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
        Ok(())
    }

    fn collect_neighbors_to_check(
        &self,
        node: NodeIndex,
        crate_index: NodeIndex,
    ) -> CgResult<Vec<ItemsCheckSemantic>> {
        Ok(self
            .iter_syn_neighbors_without_semantic_link(node)
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
            .collect::<Result<Vec<_>, _>>()?)
    }

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
        let mut current_index = self
            .get_syn_item_source_index(use_item_node_index)
            .context(add_context!("Expected source index of syn item."))?;
        // walk trough the use path
        loop {
            match use_tree {
                UseTree::Path(use_path) => {
                    let module = use_path.ident.to_string();
                    match module.as_str() {
                        "crate" => {
                            // module of current crate
                            current_index = crate_index;
                        }
                        "self" => {
                            // current module, do nothing
                        }
                        "super" => {
                            // super module
                            current_index = self
                                .get_syn_item_source_index(current_index)
                                .context(add_context!("Expected source index of syn item."))?;
                        }
                        _ => {
                            // some module, could be module of current module or local package dependency
                            if let Some((module_index, _)) = self
                                .iter_syn_neighbors(current_index)
                                .filter_map(|(n, i)| match i {
                                    Item::Mod(mod_item) => Some((n, mod_item.ident.to_string())),
                                    _ => None,
                                })
                                .find(|(_, m)| *m == module)
                            {
                                current_index = module_index;
                            } else if let Some((lib_crate_index, _)) =
                                self.iter_lib_crates().find(|(_, cf)| cf.name == module)
                            {
                                current_index = lib_crate_index;
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
                    // search for syn neighbors of current index with name of use_name
                    let import_item_name = use_name.ident.to_string();
                    let (use_name_index, _) = self
                        .iter_syn_neighbors(current_index)
                        .filter_map(|(n, i)| get_name_of_item(i).map(|(_, name)| (n, name)))
                        .find(|(_, name)| *name == import_item_name)
                        .context(add_context!(format!(
                            "Expected {import_item_name} at child of syn node {:?}.",
                            current_index
                        )))?;
                    return Ok(use_name_index);
                }
                UseTree::Rename(use_rename) => {
                    // search for syn neighbors of current index with name of use_rename
                    let import_item_name = use_rename.rename.to_string();
                    let (use_name_index, _) = self
                        .iter_syn_neighbors(current_index)
                        .filter_map(|(n, i)| get_name_of_item(i).map(|(_, name)| (n, name)))
                        .find(|(_, name)| *name == import_item_name)
                        .context(add_context!(format!(
                            "Expected {import_item_name} at child of syn node {:?}.",
                            current_index
                        )))?;
                    return Ok(use_name_index);
                }
            }
        }
    }
}

// function to flatten module structure in fusion

use std::panic;

use super::{ForgeState, ProcessingResult};
use crate::{
    CgData, add_context,
    challenge_tree::{
        CratePathFolder, EdgeType, NodeType, PathElement, RemoveSuperFolder,
        UpdateRelativePathFolder,
    },
    configuration::CgCli,
    parsing::UseTreeExt,
};

use anyhow::anyhow;
use petgraph::stable_graph::NodeIndex;
use syn::{Item, UseTree, fold::Fold};

pub struct FlattenFusionState;

impl<O: CgCli> CgData<O, FlattenFusionState> {
    pub fn flatten_fusion(mut self) -> ProcessingResult<CgData<O, ForgeState>> {
        if self.options.processing().skip_flatten {
            if self.options.verbose() {
                println!("Skipping flattening fusion...");
            }
            return Ok(self.set_state(ForgeState));
        }
        // 1. identify recursively module to flatten: flatten_module
        // 2. collect items of flatten_module: flatten_items
        // 3. check for name space collisions between flatten_module and parent module
        // 3.1 respect dependencies between flatten_module and parent module
        // 3.2 filter from flatten_items dependencies of parent module
        // 3.3 if no name space collision: copy flatten_items to parent module and modify order of items list
        // 3.3.1 replace flatten_module entry with flatten_items in order list of parent module
        // 3.4 else add flatten_module to seen list and search for another module to flatten
        // 4. remove use statements from parent module pointing to flatten_module
        // 5. relink all use statements and path with leafs pointing to flatten_module or flatten_items to parent
        //    module or new items in parent module
        // 6. remove items of flatten_module and flatten_module
        // 7. after flattening all items as much as possible, update required module content of all remaining modules
        let Some((fusion_crate, _)) = self.get_fusion_bin_crate() else {
            return Err(anyhow!(add_context!("Expected fusion bin crate.")).into());
        };

        // transform use and path statements starting with crate keyword to relative
        self.transform_use_and_path_statements_starting_with_crate_keyword_to_relative(
            fusion_crate,
        )?;

        // flatten fusion crate
        self.recursive_flatten(fusion_crate)?;

        // remove public visibility of all items in fusion crate

        // update mod content of all remaining modules
        self.update_required_mod_content(fusion_crate)?;

        Ok(self.set_state(ForgeState))
    }

    fn transform_use_and_path_statements_starting_with_crate_keyword_to_relative(
        &mut self,
        fusion_crate: NodeIndex,
    ) -> ProcessingResult<()> {
        let all_items: Vec<NodeIndex> = self
            .iter_syn_items(fusion_crate)
            .filter_map(|(n, i)| (!matches!(i, Item::Mod(_))).then_some(n))
            .collect();
        for path_item_index in all_items.iter() {
            if let Some(cloned_item) = self.clone_syn_item(*path_item_index) {
                let mut folder = CratePathFolder {
                    graph: self,
                    node: *path_item_index,
                };
                let new_item = folder.fold_item(cloned_item);
                if let Some(NodeType::SynItem(item)) = self.tree.node_weight_mut(*path_item_index) {
                    *item = new_item;
                }
            }
        }

        for use_item_index in all_items {
            if let Some(Item::Use(item_use)) = self.get_syn_item(use_item_index) {
                let new_use_item_path =
                    self.resolving_relative_source_path(use_item_index, item_use.into())?;
                let new_use_item_tree = UseTree::try_from(new_use_item_path)?;
                if let Some(NodeType::SynItem(Item::Use(use_item))) =
                    self.tree.node_weight_mut(use_item_index)
                {
                    use_item.tree = new_use_item_tree;
                }
            }
        }
        Ok(())
    }

    fn recursive_flatten(&mut self, flatten_module: NodeIndex) -> ProcessingResult<()> {
        // recursive tree traversal to mod without further mods
        let item_mod_indices: Vec<NodeIndex> = self
            .iter_syn_item_neighbors(flatten_module)
            .filter_map(|(n, i)| match i {
                Item::Mod(_) => Some(n),
                _ => None,
            })
            .collect();
        for item_mod_index in item_mod_indices {
            self.recursive_flatten(item_mod_index)?;
        }

        if self.is_crate(flatten_module) {
            // end of recursive flattening
            return Ok(());
        }

        let mut flatten_agent = FlattenAgent::new(flatten_module);

        // found module to flatten
        // 1. analyze parent
        flatten_agent.set_parent(self);

        // 2. collect flatten_items
        flatten_agent.set_flatten_items(self);

        // 3. check name space collisions
        if flatten_agent.is_name_space_conflict(self) {
            return Ok(());
        }

        // 4. collect modules, which could contain path statements, that have to change after flatten
        flatten_agent.set_sub_and_super_nodes(self);

        // 5. pre linking use and path fixing of flatten and sub items
        flatten_agent.pre_linking_use_and_path_fixing_of_sub_check_items(self);

        // 6. link flatten items to parent
        flatten_agent.link_flatten_items_to_parent(self);

        // 7. post linking use and path fixing of parent and super items
        flatten_agent.post_linking_use_and_path_fixing_of_super_check_items(self)?;

        // 8. set new order of items in parent module

        // 9. remove flatten module and unneeded use statements

        Ok(())
    }
}

#[derive(Debug)]
struct FlattenAgent {
    node: NodeIndex,
    parent: NodeIndex,
    parent_items: Vec<NodeIndex>,
    parent_use_of_flatten: Vec<NodeIndex>,
    parent_use_of_external: Vec<PathElement>,
    flatten_items: Vec<NodeIndex>,
    sub_modules: Vec<NodeIndex>,
    super_modules: Vec<NodeIndex>,
    sub_check_items: Vec<NodeIndex>,
    super_check_items: Vec<NodeIndex>,
}

impl FlattenAgent {
    fn new(flatten_module: NodeIndex) -> Self {
        Self {
            node: flatten_module,
            parent: NodeIndex::default(),
            parent_items: Vec::new(),
            parent_use_of_flatten: Vec::new(),
            parent_use_of_external: Vec::new(),
            flatten_items: Vec::new(),
            sub_modules: Vec::new(),
            super_modules: Vec::new(),
            sub_check_items: Vec::new(),
            super_check_items: Vec::new(),
        }
    }
    fn set_parent<O, S>(&mut self, graph: &CgData<O, S>) {
        let Some(parent) = graph.get_syn_module_index(self.node) else {
            unreachable!("Every module must have a parent module or crate.");
        };
        self.parent = parent;
        for (node, i) in graph.iter_syn_item_neighbors(self.parent) {
            if let Item::Use(item_use) = i {
                if let Some(module) = graph.get_path_module(node, item_use.into()) {
                    if module == self.node {
                        self.parent_use_of_flatten.push(node);
                    } else {
                        self.parent_items.push(node);
                    }
                } else {
                    panic!("{}", add_context!("Expected module of use statement."));
                }
            } else {
                self.parent_items.push(node);
            }
        }

        self.parent_use_of_external = self
            .parent_items
            .iter()
            .filter_map(|n| {
                if let Some(Item::Use(item_use)) = graph.get_syn_item(*n) {
                    match graph.get_path_leaf(*n, item_use.into()) {
                        Ok(path_leaf) => match path_leaf {
                            PathElement::ExternalGlob(_) | PathElement::ExternalItem(_) => {
                                Some(path_leaf)
                            }
                            _ => None,
                        },
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect();
    }

    fn set_flatten_items<O, S>(&mut self, graph: &CgData<O, S>) {
        self.flatten_items = graph
            .iter_syn_item_neighbors(self.node)
            .filter_map(|(n, i)| {
                if let Item::Use(item_use) = i {
                    if matches!(
                        graph.get_path_leaf(n, item_use.into()),
                        Ok(PathElement::ExternalGlob(_)) | Ok(PathElement::ExternalItem(_))
                    ) {
                        // external use statements will be processed in next step
                        Some(n)
                    } else if let Some(module) = graph.get_path_module(n, item_use.into()) {
                        // do not keep use statements, which point to parent module
                        if module != self.parent { Some(n) } else { None }
                    } else {
                        panic!("{}", add_context!("Expected module of use statement."));
                    }
                } else {
                    // keep all other items
                    Some(n)
                }
            })
            .collect();
        // remove use of external packages, which are already available in parent module
        self.flatten_items.retain(|n| {
            if let Some(Item::Use(item_use)) = graph.get_syn_item(*n) {
                match graph.get_path_leaf(*n, item_use.into()) {
                    Ok(path_leaf) => !self
                        .parent_use_of_external
                        .iter()
                        .any(|pl| *pl == path_leaf),
                    _ => {
                        panic!("{}", add_context!("Expected path leaf of use statement."));
                    }
                }
            } else {
                true
            }
        });
    }

    fn is_name_space_conflict<O, S>(&self, graph: &CgData<O, S>) -> bool {
        self.flatten_items
            .iter()
            .filter_map(|n| graph.get_ident(*n))
            .any(|flatten_ident| {
                self.parent_items
                    .iter()
                    .filter_map(|n| graph.get_ident(*n))
                    .any(|parent_ident| flatten_ident == parent_ident)
            })
    }

    fn set_sub_and_super_nodes<O: CgCli, S>(&mut self, graph: &CgData<O, S>) {
        self.sub_modules = graph
            .iter_syn_items(self.node)
            .filter_map(|(n, i)| {
                if let Item::Mod(_) = i {
                    (n != self.node).then_some(n)
                } else {
                    None
                }
            })
            .collect();
        let Some((fusion_crate, _)) = graph.get_fusion_bin_crate() else {
            panic!("{}", add_context!("Expected fusion bin crate."));
        };
        self.super_modules = graph
            .iter_syn_items(fusion_crate)
            .filter(|(n, _)| {
                graph.is_crate_or_module(*n)
                    && *n != self.node
                    && *n != self.parent
                    && !self.sub_modules.contains(n)
            })
            .map(|(n, _)| n)
            .collect();
        if self.parent != fusion_crate {
            self.super_modules.push(fusion_crate);
        }

        self.sub_check_items = self
            .flatten_items
            .iter()
            .copied()
            .chain(
                self.sub_modules
                    .iter()
                    .flat_map(|n| graph.iter_syn_item_neighbors(*n).map(|(sn, _)| sn)),
            )
            .filter(|n| !matches!(graph.get_syn_item(*n), Some(Item::Mod(_))))
            .collect();

        self.super_check_items = self
            .parent_items
            .iter()
            .copied()
            .chain(
                self.super_modules
                    .iter()
                    .flat_map(|n| graph.iter_syn_item_neighbors(*n).map(|(sn, _)| sn)),
            )
            .filter(|n| !matches!(graph.get_syn_item(*n), Some(Item::Mod(_))))
            .collect();

        self.sub_modules.push(self.node);
        self.super_modules.push(self.parent);
    }

    fn pre_linking_use_and_path_fixing_of_sub_check_items<O, S>(&self, graph: &mut CgData<O, S>) {
        // fix path statements
        for path_item_index in self.sub_check_items.iter() {
            if let Some(cloned_item) = graph.clone_syn_item(*path_item_index) {
                // remove super keyword, if path points to super modules
                let mut folder = RemoveSuperFolder {
                    graph,
                    node: *path_item_index,
                    target_mods: &self.super_modules,
                };
                let new_item = folder.fold_item(cloned_item);
                if let Some(NodeType::SynItem(item)) = graph.tree.node_weight_mut(*path_item_index)
                {
                    *item = new_item;
                }
            }
        }
        // fix use statements
        for use_item_index in self.sub_check_items.iter() {
            if let Some(Item::Use(item_use)) = graph.get_syn_item(*use_item_index) {
                if let Some(module) = graph.get_path_module(*use_item_index, item_use.into()) {
                    if self.super_modules.contains(&module) {
                        if let Some(NodeType::SynItem(Item::Use(use_item))) =
                            graph.tree.node_weight_mut(*use_item_index)
                        {
                            use_item.tree = use_item.tree.remove_super();
                        }
                    }
                }
            }
        }
    }

    fn link_flatten_items_to_parent<O, S>(&self, graph: &mut CgData<O, S>) {
        // link flatten items to parent module
        for flatten_item in self.flatten_items.iter() {
            graph
                .tree
                .add_edge(self.parent, *flatten_item, EdgeType::Syn);
        }
    }

    fn post_linking_use_and_path_fixing_of_super_check_items<O: CgCli, S>(
        &mut self,
        graph: &mut CgData<O, S>,
    ) -> ProcessingResult<()> {
        // fix path statements
        for path_item_index in self.super_check_items.iter() {
            if let Some(cloned_item) = graph.clone_syn_item(*path_item_index) {
                // update relative path to item in sub module
                let mut folder = UpdateRelativePathFolder {
                    graph,
                    node: *path_item_index,
                    target_mods: &self.sub_modules,
                };
                let new_item = folder.fold_item(cloned_item);
                if let Some(NodeType::SynItem(item)) = graph.tree.node_weight_mut(*path_item_index)
                {
                    *item = new_item;
                }
            }
        }
        // fix use statements
        for use_item_index in self.super_check_items.iter() {
            let Some(Item::Use(item_use)) = graph.get_syn_item(*use_item_index) else {
                continue;
            };
            let Some(module) = graph.get_path_module(*use_item_index, item_use.into()) else {
                continue;
            };
            if !self.sub_modules.contains(&module) {
                continue;
            }
            let new_use_item_path =
                graph.resolving_relative_source_path(*use_item_index, item_use.into())?;
            let new_use_item_tree: UseTree = new_use_item_path.try_into()?;
            if let Some(NodeType::SynItem(Item::Use(use_item))) =
                graph.tree.node_weight_mut(*use_item_index)
            {
                use_item.tree = new_use_item_tree;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;

// function to flatten module structure in fusion

use super::{ForgeState, ProcessingResult};
use crate::{add_context, challenge_tree::{PathElement, EdgeType}, configuration::CgCli, CgData};

use anyhow::anyhow;
use petgraph::stable_graph::NodeIndex;
use syn::Item;

pub struct FlattenFusionState;

impl<O: CgCli> CgData<O, FlattenFusionState> {
    pub fn flatten_fusion(mut self) -> ProcessingResult<CgData<O, ForgeState>> {
        if self.options.processing().skip_flatten {
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
        self.recursive_flatten(fusion_crate)?;

        Ok(self.set_state(ForgeState))
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

        // 4. link flatten items to parent
        flatten_agent.link_flatten_items_to_parent(self);

        // 5. collect modules, which could contain path statements, that have to change after flatten
        flatten_agent.collect_sub_and_super_modules(self);

        // 6. check path statements of flatten and parent items

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
        }
    }
    fn set_parent<O, S>(&mut self, graph: &CgData<O, S>) {
        let Some(parent) = graph.get_syn_module_index(self.node) else {
            unreachable!("Every module must have a parent module or crate.");
        };
        self.parent = parent;
        for (node, _) in graph.iter_syn_item_neighbors(self.parent) {
            if let Some(module) = graph.get_use_module(node) {
                if module == self.node {
                    self.parent_use_of_flatten.push(node);
                } else {
                    self.parent_items.push(node);
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
                    match graph.get_path_leaf(*n, item_use.into()) { Ok(path_leaf) => {
                        match path_leaf {
                            PathElement::ExternalGlob(_) | PathElement::ExternalItem(_) => {
                                Some(path_leaf)
                            }
                            _ => None,
                        }
                    } _ => {
                        None
                    }}
                } else {
                    None
                }
            })
            .collect();
    }

    fn set_flatten_items<O, S>(&mut self, graph: &CgData<O, S>) {
        self.flatten_items = graph
            .iter_syn_item_neighbors(self.node)
            .filter_map(|(n, _)| {
                if let Some(module) = graph.get_use_module(n) {
                    if module != self.parent {
                        Some(n)
                    } else {
                        None
                    }
                } else {
                    Some(n)
                }
            })
            .collect();
        // remove use of external packages, which are already available in parent module
        self.flatten_items.retain(|n| {
            if let Some(Item::Use(item_use)) = graph.get_syn_item(*n) {
                match graph.get_path_leaf(*n, item_use.into()) { Ok(path_leaf) => {
                    !self
                        .parent_use_of_external
                        .iter()
                        .any(|pl| *pl == path_leaf)
                } _ => {
                    panic!("{}", add_context!("Expected path leaf of use statement."));
                }}
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

    fn link_flatten_items_to_parent<O, S>(&self, graph: &mut CgData<O, S>) {
        for flatten_item in self.flatten_items.iter() {
            graph.tree.add_edge(self.parent, *flatten_item, EdgeType::Syn);
        }
    }

    fn collect_sub_and_super_modules<O: CgCli, S>(&mut self, graph: &CgData<O, S>) {
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
    }
}

#[cfg(test)]
mod tests;

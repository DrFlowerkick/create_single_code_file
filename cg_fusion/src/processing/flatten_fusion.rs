// function to flatten module structure in fusion

use super::{ForgeState, ProcessingResult};
use crate::{add_context, challenge_tree::PathElement, configuration::CgCli, CgData};

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
        //let use_of_external: Vec<PathElement> = self.parent_items
        self.parent_use_of_external = self
            .parent_items
            .iter()
            .filter_map(|n| {
                if let Some(Item::Use(item_use)) = graph.get_syn_item(*n) {
                    if let Ok(path_leaf) = graph.get_path_leaf(*n, item_use.into()) {
                        match path_leaf {
                            PathElement::ExternalGlob(_) | PathElement::ExternalItem(_) => {
                                Some(path_leaf)
                            }
                            _ => None,
                        }
                    } else {
                        None
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
                if let Ok(path_leaf) = graph.get_path_leaf(*n, item_use.into()) {
                    !self
                        .parent_use_of_external
                        .iter()
                        .any(|pl| *pl == path_leaf)
                } else {
                    panic!("{}", add_context!("Expected path leaf of use statement."));
                }
            } else {
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {

    use syn::Item;

    use crate::processing::flatten_fusion::FlattenAgent;

    use super::super::tests::setup_processing_test;
    use super::*;

    #[test]
    fn test_set_parent() {
        // preparation
        let cg_data = setup_processing_test(true)
            .add_challenge_dependencies()
            .unwrap()
            .add_src_files()
            .unwrap()
            .expand_use_statements()
            .unwrap()
            .path_minimizing_of_use_and_path_statements()
            .unwrap()
            .link_impl_blocks_with_corresponding_item()
            .unwrap()
            .link_required_by_challenge()
            .unwrap()
            .check_impl_blocks()
            .unwrap()
            .process_external_dependencies()
            .unwrap()
            .fuse_challenge()
            .unwrap();

        let (fusion_node, _) = cg_data.get_fusion_bin_crate().unwrap();

        // test mod MapPoint
        let map_point_mod = cg_data
            .iter_syn_items(fusion_node)
            .find_map(|(n, i)| {
                if let Item::Mod(item_mod) = i {
                    (item_mod.ident == "my_map_point").then_some(n)
                } else {
                    None
                }
            })
            .unwrap();
        // fusion of my_map_point does not contain further mod
        assert!(!cg_data
            .iter_syn_item_neighbors(map_point_mod)
            .any(|(_, i)| matches!(i, Item::Mod(_))));
        let mut flatten_agent = FlattenAgent::new(map_point_mod);

        // action to test
        flatten_agent.set_parent(&cg_data);

        assert_eq!(
            cg_data
                .get_verbose_name_of_tree_node(flatten_agent.parent)
                .unwrap(),
            "my_map_two_dim (Mod)"
        );

        let items: Vec<String> = flatten_agent
            .parent_items
            .iter()
            .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
            .collect();
        assert_eq!(
            items,
            [
                "my_map_point (Mod)",
                "MyMap2D (Struct)",
                "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>",
                "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>"
            ]
        );

        let use_of_flatten: Vec<String> = flatten_agent
            .parent_use_of_flatten
            .iter()
            .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
            .collect();
        assert_eq!(use_of_flatten, ["MapPoint (Use)"]);

        assert!(flatten_agent.parent_use_of_external.is_empty());

        // test mod Action
        let action_mod = cg_data
            .iter_syn_items(fusion_node)
            .find_map(|(n, i)| {
                if let Item::Mod(item_mod) = i {
                    (item_mod.ident == "action").then_some(n)
                } else {
                    None
                }
            })
            .unwrap();
        // fusion of my_map_point does not contain further mod
        assert!(!cg_data
            .iter_syn_item_neighbors(action_mod)
            .any(|(_, i)| matches!(i, Item::Mod(_))));
        let mut flatten_agent = FlattenAgent::new(action_mod);

        // action to test
        flatten_agent.set_parent(&cg_data);

        assert_eq!(
            cg_data
                .get_verbose_name_of_tree_node(flatten_agent.parent)
                .unwrap(),
            "cg_fusion_binary_test (Mod)"
        );

        let items: Vec<String> = flatten_agent
            .parent_items
            .iter()
            .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
            .collect();
        assert_eq!(
            items,
            [
                "action (Mod)",
                "fmt (Use)",
                "X (Const)",
                "Y (Const)",
                "N (Const)",
                "Value (Enum)",
                "impl fmt::Display for Value",
                "Go (Struct)",
                "impl Default for Go",
                "impl Go",
                "MyMap2D (Use)"
            ]
        );

        let use_of_flatten: Vec<String> = flatten_agent
            .parent_use_of_flatten
            .iter()
            .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
            .collect();
        assert_eq!(use_of_flatten, ["Action (Use)"]);

        assert_eq!(flatten_agent.parent_use_of_external.len(), 1);
        assert!(matches!(
            flatten_agent.parent_use_of_external[0],
            PathElement::ExternalItem(_)
        ));
        if let PathElement::ExternalItem(ref item) = flatten_agent.parent_use_of_external[0] {
            assert!(item == "fmt");
        }
    }

    #[test]
    fn test_set_flatten_items() {
        // preparation
        let cg_data = setup_processing_test(true)
            .add_challenge_dependencies()
            .unwrap()
            .add_src_files()
            .unwrap()
            .expand_use_statements()
            .unwrap()
            .path_minimizing_of_use_and_path_statements()
            .unwrap()
            .link_impl_blocks_with_corresponding_item()
            .unwrap()
            .link_required_by_challenge()
            .unwrap()
            .check_impl_blocks()
            .unwrap()
            .process_external_dependencies()
            .unwrap()
            .fuse_challenge()
            .unwrap();

        let (fusion_node, _) = cg_data.get_fusion_bin_crate().unwrap();

        // test mod MapPoint
        let map_point_mod = cg_data
            .iter_syn_items(fusion_node)
            .find_map(|(n, i)| {
                if let Item::Mod(item_mod) = i {
                    (item_mod.ident == "my_map_point").then_some(n)
                } else {
                    None
                }
            })
            .unwrap();
        // fusion of my_map_point does not contain further mod
        assert!(!cg_data
            .iter_syn_item_neighbors(map_point_mod)
            .any(|(_, i)| matches!(i, Item::Mod(_))));
        let mut flatten_agent = FlattenAgent::new(map_point_mod);
        flatten_agent.set_parent(&cg_data);

        // action to test
        flatten_agent.set_flatten_items(&cg_data);

        let flatten_items: Vec<String> = flatten_agent
            .flatten_items
            .iter()
            .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
            .collect();

        assert_eq!(
            flatten_items,
            [
                "MapPoint (Struct)",
                "impl<constX:usize,constY:usize> MapPoint<X,Y>",
            ]
        );

        // test mod Action
        let action_mod = cg_data
            .iter_syn_items(fusion_node)
            .find_map(|(n, i)| {
                if let Item::Mod(item_mod) = i {
                    (item_mod.ident == "action").then_some(n)
                } else {
                    None
                }
            })
            .unwrap();
        // fusion of my_map_point does not contain further mod
        assert!(!cg_data
            .iter_syn_item_neighbors(action_mod)
            .any(|(_, i)| matches!(i, Item::Mod(_))));
        let mut flatten_agent = FlattenAgent::new(action_mod);
        flatten_agent.set_parent(&cg_data);

        // action to test
        flatten_agent.set_flatten_items(&cg_data);

        let flatten_items: Vec<String> = flatten_agent
            .flatten_items
            .iter()
            .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
            .collect();

        assert_eq!(
            flatten_items,
            [
                "MapPoint (Use)",
                "Action (Struct)",
                "impl Display for Action",
                "impl Action"
            ]
        );
    }
}

// function to flatten module structure in fusion

use super::{ForgeState, ProcessingResult};
use crate::{add_context, configuration::CgCli, CgData};

use anyhow::{anyhow, Context};
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

        // found module to flatten
        // 1. analyze parent
        let Some(parent_module) = self.get_syn_module_index(flatten_module) else {
            unreachable!("Every module must have a parent module or crate.");
        };
        let mut parent_items: Vec<NodeIndex> = Vec::new();
        let mut parent_use_of_flatten: Vec<NodeIndex> = Vec::new();
        for (node, _) in self.iter_syn_item_neighbors(parent_module) {
            if let Some(module) = self.get_use_module(node) {
                if module == flatten_module {
                    parent_use_of_flatten.push(node);
                } else {
                    parent_items.push(node);
                }
            } else {
                parent_items.push(node);
            }
        }
        //let parent_use_of_external: Vec<>
        // 2. collect flatten_items
        let mut flatten_items: Vec<NodeIndex> = self
            .iter_syn_item_neighbors(flatten_module)
            .filter_map(|(n, _)| {
                if let Some(module) = self.get_use_module(n) {
                    if module != parent_module {
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
        flatten_items.retain(|n| todo!("identify if external use, than use target, than check if use target is available in parent_module."));

        Ok(())
    }
}

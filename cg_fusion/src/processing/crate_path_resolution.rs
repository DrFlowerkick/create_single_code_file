// use and path statements may start with crate. Since cg-fusion fuses all crates into one, the crate keyword is
// not needed. may lead to unexpected behavior. Therefore, this function removes the crate keyword from
// use and path statements.

use super::{ProcessingError, ProcessingImplBlocksState, ProcessingResult};
use crate::{
    challenge_tree::NodeType,
    configuration::CgCli,
    parsing::{SourcePath, UseTreeExtras},
    CgData,
};

use petgraph::stable_graph::NodeIndex;
use syn::{fold::Fold, Item, Path, UseTree};

pub struct ProcessingCrateUseAndPathState;

impl<O: CgCli> CgData<O, ProcessingCrateUseAndPathState> {
    pub fn remove_crate_keyword_from_use_and_path_statements(
        mut self,
    ) -> ProcessingResult<CgData<O, ProcessingImplBlocksState>> {
        // 1. remove crate keyword from use statements
        let use_items_with_crate_keyword: Vec<(NodeIndex, SourcePath)> = self
            .iter_crates()
            .flat_map(|(n, _, _)| self.iter_syn_items(n))
            .filter_map(|(n, i)| {
                if let syn::Item::Use(use_item) = i {
                    if use_item.tree.is_use_tree_root_crate_keyword() {
                        Some((n, SourcePath::from(use_item)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        for (use_item_index, use_item_path) in use_items_with_crate_keyword {
            let new_use_item_path =
                self.resolving_crate_source_path(use_item_index, use_item_path)?;
            let new_use_item_tree: UseTree = new_use_item_path.try_into()?;
            if let Some(NodeType::SynItem(Item::Use(use_item))) =
                self.tree.node_weight_mut(use_item_index)
            {
                use_item.tree = new_use_item_tree;
            }
        }

        // 2. remove crate keyword from path statements
        let all_syn_items: Vec<NodeIndex> = self
            .iter_crates()
            .flat_map(|(n, _, _)| self.iter_syn(n).map(|(n, _)| n))
            .collect();
        for syn_index in all_syn_items {
            if let Some(cloned_item) = self.clone_syn_item(syn_index) {
                let mut folder = CratePathFolder {
                    graph: &self,
                    node: syn_index,
                };
                let new_item = folder.fold_item(cloned_item);
                if let Some(NodeType::SynItem(item)) = self.tree.node_weight_mut(syn_index) {
                    *item = new_item;
                }
            }
            if let Some(cloned_impl_item) = self.clone_syn_impl_item(syn_index) {
                let mut folder = CratePathFolder {
                    graph: &self,
                    node: syn_index,
                };
                let new_impl_item = folder.fold_impl_item(cloned_impl_item);
                if let Some(NodeType::SynImplItem(impl_item)) = self.tree.node_weight_mut(syn_index)
                {
                    *impl_item = new_impl_item;
                }
            }
            if let Some(cloned_trait_item) = self.clone_syn_trait_item(syn_index) {
                let mut folder = CratePathFolder {
                    graph: &self,
                    node: syn_index,
                };
                let new_trait_item = folder.fold_trait_item(cloned_trait_item);
                if let Some(NodeType::SynTraitItem(trait_item)) =
                    self.tree.node_weight_mut(syn_index)
                {
                    *trait_item = new_trait_item;
                }
            }
        }

        Ok(CgData {
            state: ProcessingImplBlocksState,
            options: self.options,
            tree: self.tree,
        })
    }

    pub(crate) fn resolving_crate_source_path(
        &self,
        item_index_of_path: NodeIndex,
        source_path: SourcePath,
    ) -> ProcessingResult<SourcePath> {
        unimplemented!()
    }
}

pub struct CratePathFolder<'a, O: CgCli> {
    graph: &'a CgData<O, ProcessingCrateUseAndPathState>,
    node: NodeIndex,
}

impl<O: CgCli> Fold for CratePathFolder<'_, O> {
    fn fold_path(&mut self, path: Path) -> Path {
        let source_path = SourcePath::from(&path);
        let resolved_path = self
            .graph
            .resolving_crate_source_path(self.node, source_path)
            .expect("resolving crate source path failed");
        Path::try_from(resolved_path).expect("resolving crate source path failed")
    }
}

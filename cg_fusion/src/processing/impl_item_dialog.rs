// dialog to include or exclude impl items, which at current state are not marked
// as required by challenge, of impl blocks, which are marked as required for
// challenge.

use super::{ProcessedState, ProcessingResult};
use crate::{
    add_context,
    challenge_tree::{EdgeType, NodeType, PathElement, SourcePathWalker},
    configuration::CgCliImplDialog,
    CgData,
};
use anyhow::{anyhow, Context};
use petgraph::stable_graph::NodeIndex;
use std::collections::HashSet;

pub struct ProcessingImplItemDialogState;

impl<O: CgCliImplDialog> CgData<O, ProcessingImplItemDialogState> {
    pub fn impl_item_dialog(mut self) -> ProcessingResult<CgData<O, ProcessedState>> {
        let mut seen_dialog_items: HashSet<NodeIndex> = HashSet::new();
        let mut seen_check_items: HashSet<NodeIndex> = self
            .iter_items_required_by_challenge()
            .map(|(n, _)| n)
            .collect();
        let impl_options = self.map_impl_config_options_to_node_indices()?;
        while let Some(dialog_item) =
            self.find_impl_item_without_required_link_in_required_impl_block(&seen_dialog_items)
        {
            seen_dialog_items.insert(dialog_item);
            let impl_block_index = self
                .get_parent_index_by_edge_type(dialog_item, EdgeType::Syn)
                .unwrap();
            println!(
                "Found '{}' of required '{}'.",
                self.get_verbose_name_of_tree_node(dialog_item)?,
                self.get_verbose_name_of_tree_node(impl_block_index)?
            );
            // ToDo: Dialog setup. We want to test dialogs with mock. Probably will use dialoguer
            // Select for cmd dialog.
            // Dialog prompt: Mark '{}' as required? ('Esc' or 'q' quits cg_fusion), dialog_item
            // Options:
            // yes
            // no
            // always yes for all remaining impl items of '{}', impl_block_index
            // always no for all remaining impl items of '{}', impl_block_index
            // show '{}', dialog_item
            // show possible usage of '{}', dialog_item
            // We need enum UserInput with Yes, No, AlwaysYes, AlwaysNo
            // dialog returns Option<UserInput>; if None, quit cg_fusion with CgError::UserQuitDialog
            // We need cli to skip dialog with either AlwaysYes or AlwaysNo; Both could be a
            // Vec<String> with names of struct, enum, union. We check, if these names are unambiguous.
            // If no, cg_fusion quits with message. name structure is
            // crate_name::module_name_1::...::module_name_n::item_name
            // no wild cards, crate_name and module_names are optional.
            // We need cli option to save dialog results in a config file and an option to use config file
            // for impl fn
            let user_input: bool = unimplemented!("Create dialog fn");
            if user_input {
                self.add_required_by_challenge_link(impl_block_index, dialog_item)?;
                self.check_path_items_for_challenge(dialog_item, &mut seen_check_items)?;
            }
        }
        Ok(CgData {
            state: ProcessedState,
            options: self.options,
            tree: self.tree,
        })
    }

    fn find_impl_item_without_required_link_in_required_impl_block(
        &self,
        seen_dialog_items: &HashSet<NodeIndex>,
    ) -> Option<NodeIndex> {
        self.iter_impl_items_without_required_link_in_required_impl_block()
            .filter_map(|(n, _)| (!seen_dialog_items.contains(&n)).then_some(n))
            .next()
    }
}

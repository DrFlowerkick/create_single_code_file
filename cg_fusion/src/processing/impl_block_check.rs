// Checking impl items of required impl blocks. If config options do not process
// the impl item, a dialog prompts the user to decide, if the impl item should be
// include in or exclude from the challenge.

use super::{ProcessedState, ProcessingError, ProcessingResult};
use crate::{add_context, challenge_tree::EdgeType, configuration::CgCliImplDialog, CgData};
use anyhow::anyhow;
use anyhow::Result as AnyResult;
use inquire::{ui::RenderConfig, Select};
use mockall::{automock, predicate::*};
use petgraph::stable_graph::NodeIndex;
use std::collections::hash_map::Entry;
use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Write},
    io,
};
use syn::spanned::Spanned;

pub struct ProcessingImplItemDialogState;

impl<O: CgCliImplDialog> CgData<O, ProcessingImplItemDialogState> {
    pub fn check_impl_blocks_required_by_challenge(
        mut self,
    ) -> ProcessingResult<CgData<O, ProcessedState>> {
        let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();
        let mut seen_check_items: HashSet<NodeIndex> = self
            .iter_items_required_by_challenge()
            .map(|(n, _)| n)
            .collect();
        let impl_options = self.map_impl_config_options_to_node_indices()?;
        while let Some(impl_item) = {
            let next_item_option = self
                .iter_impl_items_without_required_link_in_required_impl_block()
                .filter_map(|(n, _)| (!seen_impl_items.contains_key(&n)).then_some(n))
                .next();
            next_item_option
        } {
            let impl_block = self
                .get_parent_index_by_edge_type(impl_item, EdgeType::Syn)
                .unwrap();
            if let Some(include) = impl_options.get(&impl_item) {
                if *include {
                    self.add_required_by_challenge_link(impl_block, impl_item)?;
                    self.check_path_items_for_challenge(impl_item, &mut seen_check_items)?;
                } else if self.options.verbose() {
                    println!(
                        "Excluding impl item '{}'",
                        self.get_verbose_name_of_tree_node(impl_item)?
                    );
                }
                seen_impl_items.insert(impl_item, *include);
                continue;
            }
            let mut selection_handler = SelectionCli::new(std::io::stdout());
            let user_input = self.impl_item_dialog(
                impl_item,
                impl_block,
                &mut selection_handler,
                &mut seen_impl_items,
            )?;
            if user_input {
                self.add_required_by_challenge_link(impl_block, impl_item)?;
                self.check_path_items_for_challenge(impl_item, &mut seen_check_items)?;
            }
            seen_impl_items.insert(impl_item, user_input);
        }
        Ok(CgData {
            state: ProcessedState,
            options: self.options,
            tree: self.tree,
        })
    }

    fn impl_item_dialog(
        &self,
        dialog_item: NodeIndex,
        impl_block: NodeIndex,
        selection_handler: &mut impl SelectionDialog<String, String>,
        seen_impl_items: &mut HashMap<NodeIndex, bool>,
    ) -> ProcessingResult<bool> {
        loop {
            match self.impl_item_selection(dialog_item, impl_block, selection_handler)? {
                UserSelection::IncludeItem => return Ok(true),
                UserSelection::ExcludeItem => return Ok(false),
                UserSelection::IncludeAllItemsOfImplBlock => {
                    for (item_index, _) in self
                        .iter_syn_impl_item(impl_block)
                        .filter(|(n, _)| !self.is_required_by_challenge(*n))
                    {
                        seen_impl_items.insert(item_index, true);
                    }
                    return Ok(true);
                }
                UserSelection::ExcludeAllItemsOfImplBlock => {
                    for (item_index, _) in self
                        .iter_syn_impl_item(impl_block)
                        .filter(|(n, _)| !self.is_required_by_challenge(*n))
                    {
                        if let Entry::Vacant(entry) = seen_impl_items.entry(item_index) {
                            entry.insert(false);
                        }
                    }
                    return Ok(false);
                }
                UserSelection::ShowItem => {
                    let mut message = String::new();
                    // extracting source code span of dialog item
                    if let Some(impl_item) = self.get_syn_impl_item(dialog_item) {
                        if let Some(src_file) = self.get_src_file_containing_item(dialog_item) {
                            let span = impl_item.span();
                            if let Some(impl_item_source) = span.source_text() {
                                writeln!(&mut message,
                                    "\n{}:{}:{}\n{}\n",
                                    src_file.path,
                                    span.start().line,
                                    span.start().column + 1,
                                    impl_item_source,
                                )?;
                            }
                        }
                    }
                    if message.is_empty() {
                        message = format!(
                            "Something went wrong with extracting source code span of '{}'",
                            self.get_verbose_name_of_tree_node(dialog_item)?
                        );
                    }
                    selection_handler.write_output(message)?;
                }
                UserSelection::ShowUsageOfItem => {
                    let mut message = String::new();
                    // extracting source code span of dialog item
                    for (node_index, src_span, ident) in self.get_possible_usage_of_impl_item_in_required_items(dialog_item).iter() {
                        if let Some(src_file) = self.get_src_file_containing_item(*node_index) {
                            let span = ident.span();
                            if let Some(usage_of_impl_item_source) = src_span.source_text() {
                                writeln!(&mut message,
                                    "\n{}:{}:{}\n{}\n",
                                    src_file.path,
                                    span.start().line,
                                    span.start().column + 1,
                                    usage_of_impl_item_source,
                                )?;
                            }
                        }
                    }
                    if message.is_empty() {
                        message = format!(
                            "Something went wrong with extracting source code span using '{}'",
                            self.get_verbose_name_of_tree_node(dialog_item)?
                        );
                    }
                    selection_handler.write_output(message)?;
                },
                UserSelection::Quit => return Err(ProcessingError::UserCanceledDialog),
            }
        }
    }

    fn impl_item_selection(
        &self,
        dialog_item: NodeIndex,
        impl_block: NodeIndex,
        selection_handler: &mut impl SelectionDialog<String, String>,
    ) -> ProcessingResult<UserSelection> {
        let dialog_item_name = self.get_verbose_name_of_tree_node(dialog_item)?;
        let impl_block_name = self.get_verbose_name_of_tree_node(impl_block)?;
        let prompt = format!(
            "Found '{}' of required '{}'.",
            dialog_item_name, impl_block_name
        );
        let options = vec![
            format!("Include '{}'.", dialog_item_name),
            format!("Exclude '{}'.", dialog_item_name),
            format!("Include all items of '{}'.", impl_block_name),
            format!("Exclude all items of '{}'.", impl_block_name),
            format!("Show code of '{}'.", dialog_item_name),
            format!("Show usage of '{}'.", dialog_item_name),
        ];
        if let Some(selection) = selection_handler.select_option(&prompt, "", options.clone())? {
            let user_selection =
                UserSelection::try_from(options.iter().position(|o| *o == selection))?;
            return Ok(user_selection);
        }
        Ok(UserSelection::Quit)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum UserSelection {
    IncludeItem,
    ExcludeItem,
    IncludeAllItemsOfImplBlock,
    ExcludeAllItemsOfImplBlock,
    ShowItem,
    ShowUsageOfItem,
    Quit,
}

impl TryFrom<Option<usize>> for UserSelection {
    type Error = anyhow::Error;

    fn try_from(value: Option<usize>) -> Result<Self, Self::Error> {
        if let Some(selection) = value {
            match selection {
                0 => Ok(UserSelection::IncludeItem),
                1 => Ok(UserSelection::ExcludeItem),
                2 => Ok(UserSelection::IncludeAllItemsOfImplBlock),
                3 => Ok(UserSelection::ExcludeAllItemsOfImplBlock),
                4 => Ok(UserSelection::ShowItem),
                5 => Ok(UserSelection::ShowUsageOfItem),
                _ => Err(anyhow!(
                    "{}",
                    add_context!("Expected selection in range of UserSelection.")
                )),
            }
        } else {
            Ok(UserSelection::Quit)
        }
    }
}

#[automock]
trait SelectionDialog<S: Display + 'static, M: Display + 'static> {
    fn select_option(&self, prompt: &str, help: &str, options: Vec<S>) -> AnyResult<Option<S>>;
    fn write_output(&mut self, message: M) -> AnyResult<()>;
}

struct SelectionCli<W: io::Write, S: Display + 'static, M: Display + 'static> {
    writer: W,
    _select_display_type: std::marker::PhantomData<S>,
    _message_display_type: std::marker::PhantomData<M>,
}

impl<W: io::Write> SelectionCli<W, String, String> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            _select_display_type: std::marker::PhantomData,
            _message_display_type: std::marker::PhantomData,
        }
    }
}

impl<S: Display + 'static, M: Display + 'static, W: io::Write> SelectionDialog<S, M>
    for SelectionCli<W, S, M>
{
    fn select_option(&self, prompt: &str, help: &str, options: Vec<S>) -> AnyResult<Option<S>> {
        let selected_item = Select::new(prompt, options)
            .with_render_config(RenderConfig::default_colored())
            .with_help_message(help)
            .prompt_skippable()?;
        Ok(selected_item)
    }
    fn write_output(&mut self, message: M) -> AnyResult<()> {
        write!(self.writer, "{}", message)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

// Checking impl items of required impl blocks. If config options do not process
// the impl item, a dialog prompts the user to decide, if the impl item should be
// include in or exclude from the challenge.

mod inquire_dialog;

use super::{ProcessingError, ProcessingRequiredExternals, ProcessingResult};
use crate::{
    CgData, add_context,
    challenge_tree::NodeType,
    configuration::CgCliImplDialog,
    utilities::{CgDialog, DialogCli, clean_absolute_utf8, current_dir_utf8, get_relative_path},
};
use anyhow::{Context, anyhow};
use cargo_metadata::camino::Utf8PathBuf;
use inquire_dialog::{
    ConfigFilePathValidator, DialogImplBlockSelection, DialogImplBlockWithTraitSelection,
    DialogImplItemSelection,
};
use petgraph::stable_graph::NodeIndex;
use std::collections::hash_map::Entry;
use std::{
    collections::{HashMap, HashSet},
    fmt::Write as FmtWrite,
    fs,
    io::Write,
};
use syn::{Item, spanned::Spanned};
use toml_edit::{DocumentMut, value};

pub struct ProcessingImplItemDialogState;

const IMPL_CONFIG_TOML_TEMPLATE: &str = r#"# impl config file in TOML format to configure impl items of specific impl blocks to
# include in or exclude from challenge.
# file structure:
# [impl_items]
# include = [include_item_1, include_item_2]
# exclude = [exclude_item_1, exclude_item_2]
# [impl_blocks]
# include = [include_impl_block_1, include_impl_block_2]
# exclude = [exclude_impl_block_1, exclude_impl_block_2]
#
# If in conflict with other impl options (item or block), the 'include' option always wins.
#
# --- impl items of impl blocks ---
# impl items are identified by their plain name, e.g.
# fn my_function() --> my_function
# const MY_CONST --> MY_CONST
# If the name of the impl item is ambiguous (e.g. push(), next(), etc.), add the fully
# qualified name of the impl block containing the impl item. Use the following naming
# schema:
# impl_item_name@fully_qualified_name_of_impl_block
#
# A fully qualified name of an impl block consists of up to four components:
# 1. impl with lifetime and type parameters if applicable, e.g. impl<'a, T: Display>
# 2. if impl with a trait, than path to trait with lifetime and type parameters if applicable and 'for' keyword, e.g.
#    convert::From<&str> for
# 3. path to user defined type with lifetime and type parameters if applicable referenced by impl
#    block, e.g. map::TwoDim<X, Y>
# 4. if impl has a where clause, than where clause for type parameters, e.g. where D: Display
#
# Specify the components without any whitespace with the exception of one space between trait and
# 'for' keyword. The components are separated each by one space.
# Example 1: impl<constX:usize,constY:usize> map::TwoDim<X,Y>
# Example 2: impl<'a> From<&'astr> for FooType<'a>
# Example 3: impl<D> MyPrint for MyType<D> whereD:Display
#
# Usage of wildcard '*' for impl item name is possible, but requires a fully qualified name of an
# impl block, e.g.: *@impl StructFoo
# This will include all impl items of the corresponding impl block(s)
#
# --- impl block ---
# cg-fusion uses a simple approach to identify required items of src code, which is in most cases not
# capable of identifying dependencies on traits like Display or From. To include these traits in the
# fusion of challenge, add all required impl blocks by their fully qualified name (see above) to the
# configuration. If an impl block with a trait is included, than all items of the impl block will be
# required by fusion of challenge.
# If you configure an impl block without a trait, the impl items of this block will be added to the
# impl user dialog. If you want to avoid this dialog, add the required impl items with the above impl
# item include options to the configuration. In this case you do not need to add the corresponding
# impl block to the configuration, because every impl block, which contains required items, will be
# pulled into the fusion automatically.
[impl_items]
include = []
exclude = []
[impl_blocks]
include = []
exclude = []
"#;

impl<O: CgCliImplDialog> CgData<O, ProcessingImplItemDialogState> {
    pub fn check_impl_blocks(mut self) -> ProcessingResult<CgData<O, ProcessingRequiredExternals>> {
        let mut seen_dialog_items: HashMap<NodeIndex, bool> = HashMap::new();
        let impl_options = self.map_impl_config_options_to_node_indices()?;
        let mut dialog_handler = DialogCli::new(std::io::stdout());
        let mut seen_check_items: HashSet<NodeIndex> = self
            .iter_items_required_by_challenge()
            .map(|(n, _)| n)
            .collect();
        let mut got_user_input = false;
        while let Some((dialog_item, required_node)) = {
            let next_item_option = self
                .iter_impl_items_without_required_link_in_required_impl_blocks()
                .chain(self.iter_impl_blocks_without_required_link_of_required_items())
                .find(|(n, _)| (!seen_dialog_items.contains_key(n)));
            next_item_option
        } {
            let processing = if (self.options.processing().force_challenge_items
                && self.is_challenge_item(dialog_item))
                || (!self.options.processing().unambiguous_impl_items_dialog
                    && self.is_unambiguous_impl_item(dialog_item))
            {
                Some(true)
            } else {
                match (
                    impl_options.get(&dialog_item),
                    self.options.processing().process_all_impl_items,
                ) {
                    (Some(true), _) | (_, Some(true)) => Some(true),
                    (Some(false), _) | (_, Some(false)) => Some(false),
                    _ => None,
                }
            };
            match processing {
                Some(true) => {
                    self.add_required_by_challenge_link(required_node, dialog_item)?;
                    self.add_challenge_links_for_referenced_nodes_of_item(
                        dialog_item,
                        &mut seen_check_items,
                    )?;
                    seen_dialog_items.insert(dialog_item, true);
                }
                Some(false) => {
                    if self.options.verbose() {
                        println!(
                            "Excluding impl item '{}'",
                            self.get_verbose_name_of_tree_node(dialog_item)?
                        );
                    }
                    seen_dialog_items.insert(dialog_item, false);
                }
                None => {
                    // no  configuration for dialog_item -> do user dialog
                    got_user_input = true;
                    let user_input =
                        self.impl_dialog(dialog_item, required_node, &mut dialog_handler)?;
                    for (node, selection) in user_input {
                        if selection {
                            self.add_required_by_challenge_link(required_node, node)?;
                            self.add_challenge_links_for_referenced_nodes_of_item(
                                node,
                                &mut seen_check_items,
                            )?;
                            seen_dialog_items.insert(node, true);
                        } else if let Entry::Vacant(entry) = seen_dialog_items.entry(node) {
                            entry.insert(false);
                        }
                    }
                }
            }
        }
        // if at least once user input was required, show dialog to save impl config file.
        if got_user_input {
            if let Some((toml_path, toml_content)) =
                self.impl_config_toml_dialog(&mut dialog_handler, &seen_dialog_items)?
            {
                let confirmation = if toml_path.exists() {
                    let prompt = format!("Overwriting existing impl config file '{}'?", toml_path);
                    let help = "Default is not overwriting (N).";
                    dialog_handler.confirm(&prompt, help, false)?
                } else {
                    true
                };
                if confirmation {
                    let mut file = fs::File::create(toml_path)?;
                    file.write_all(toml_content.as_bytes())?;
                } else if self.options.verbose() {
                    println!("Skipping saving impl config to '{}'.", toml_path);
                }
            }
        }
        Ok(self.set_state(ProcessingRequiredExternals))
    }

    fn impl_dialog(
        &self,
        dialog_item: NodeIndex,
        required_node: NodeIndex,
        dialog_handler: &mut impl CgDialog<String, String>,
    ) -> ProcessingResult<Vec<(NodeIndex, bool)>> {
        if self.is_syn_impl_item(dialog_item) {
            self.impl_item_dialog(dialog_item, required_node, dialog_handler)
        } else {
            match self.tree.node_weight(dialog_item) {
                Some(NodeType::SynItem(Item::Impl(item_impl))) => {
                    if item_impl.trait_.is_some() {
                        self.impl_block_with_trait_dialog(
                            dialog_item,
                            required_node,
                            dialog_handler,
                        )
                    } else {
                        self.impl_block_dialog(dialog_item, required_node, dialog_handler)
                    }
                }
                _ => Err(anyhow!(add_context!("Expected either impl item or block")).into()),
            }
        }
    }

    fn impl_item_dialog(
        &self,
        impl_item: NodeIndex,
        impl_block: NodeIndex,
        dialog_handler: &mut impl CgDialog<String, String>,
    ) -> ProcessingResult<Vec<(NodeIndex, bool)>> {
        loop {
            match self.impl_item_selection(impl_item, impl_block, dialog_handler)? {
                DialogImplItemSelection::IncludeItem => return Ok(vec![(impl_item, true)]),
                DialogImplItemSelection::ExcludeItem => return Ok(vec![(impl_item, false)]),
                DialogImplItemSelection::IncludeAllItemsOfImplBlock => {
                    return Ok(self
                        .iter_syn_impl_item(impl_block)
                        .filter_map(|(n, _)| {
                            (!self.is_required_by_challenge(n)).then_some((n, true))
                        })
                        .collect());
                }
                DialogImplItemSelection::ExcludeAllItemsOfImplBlock => {
                    return Ok(self
                        .iter_syn_impl_item(impl_block)
                        .filter_map(|(n, _)| {
                            (!self.is_required_by_challenge(n)).then_some((n, false))
                        })
                        .collect());
                }
                DialogImplItemSelection::ShowItem => {
                    let mut message = String::new();
                    // extracting source code span of dialog item
                    if let Some(ii) = self.get_syn_impl_item(impl_item) {
                        if let Some(src_file) = self.get_src_file_containing_item(impl_item) {
                            let span = ii.span();
                            if let Some(impl_item_source) = span.source_text() {
                                writeln!(
                                    &mut message,
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
                            self.get_verbose_name_of_tree_node(impl_item)?
                        );
                    }
                    dialog_handler.write_output(message)?;
                }
                DialogImplItemSelection::ShowUsageOfItem => {
                    let mut message = String::new();
                    // extracting source code span of dialog item
                    for (node_index, src_span, ident) in self
                        .get_possible_usage_of_impl_item_in_required_items(impl_item)
                        .iter()
                    {
                        if let Some(src_file) = self.get_src_file_containing_item(*node_index) {
                            let span = ident.span();
                            if let Some(usage_of_impl_item_source) = src_span.source_text() {
                                writeln!(
                                    &mut message,
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
                            self.get_verbose_name_of_tree_node(impl_item)?
                        );
                    }
                    dialog_handler.write_output(message)?;
                }
                DialogImplItemSelection::Quit => return Err(ProcessingError::UserCanceledDialog),
            }
        }
    }

    fn impl_item_selection(
        &self,
        impl_item: NodeIndex,
        impl_block: NodeIndex,
        dialog_handler: &mut impl CgDialog<String, String>,
    ) -> ProcessingResult<DialogImplItemSelection> {
        let impl_item_name = self.get_verbose_name_of_tree_node(impl_item)?;
        let impl_block_name = self.get_verbose_name_of_tree_node(impl_block)?;
        let prompt = format!(
            "Found '{}' of required '{}'.",
            impl_item_name, impl_block_name
        );
        let options = vec![
            format!("Include '{}'.", impl_item_name),
            format!("Exclude '{}'.", impl_item_name),
            format!("Include all items of '{}'.", impl_block_name),
            format!("Exclude all items of '{}'.", impl_block_name),
            format!("Show code of '{}'.", impl_item_name),
            format!("Show usage of '{}'.", impl_item_name),
        ];
        if let Some(selection) = dialog_handler.select_option(
            &prompt,
            "↑↓ to move, enter to select, type to filter, and esc to quit.",
            options.clone(),
        )? {
            let user_selection =
                DialogImplItemSelection::try_from(options.iter().position(|o| *o == selection))?;
            return Ok(user_selection);
        }
        Ok(DialogImplItemSelection::Quit)
    }

    fn impl_block_dialog(
        &self,
        impl_block: NodeIndex,
        required_node: NodeIndex,
        dialog_handler: &mut impl CgDialog<String, String>,
    ) -> ProcessingResult<Vec<(NodeIndex, bool)>> {
        loop {
            match self.impl_block_selection(impl_block, required_node, dialog_handler)? {
                DialogImplBlockSelection::IncludeImplBlock => return Ok(vec![(impl_block, true)]),
                DialogImplBlockSelection::ExcludeImplBlock => return Ok(vec![(impl_block, false)]),
                DialogImplBlockSelection::IncludeAllItemsOfImplBlock => {
                    // by adding impl items of impl block, impl block will automatically link as required
                    return Ok(self
                        .iter_syn_impl_item(impl_block)
                        .filter_map(|(n, _)| {
                            (!self.is_required_by_challenge(n)).then_some((n, true))
                        })
                        .collect());
                }
                DialogImplBlockSelection::ShowImplBlock => {
                    let mut message = String::new();
                    // extracting source code span of dialog item
                    if let Some(item) = self.get_syn_item(impl_block) {
                        if let Some(src_file) = self.get_src_file_containing_item(impl_block) {
                            let span = item.span();
                            if let Some(impl_item_source) = span.source_text() {
                                writeln!(
                                    &mut message,
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
                            self.get_verbose_name_of_tree_node(impl_block)?
                        );
                    }
                    dialog_handler.write_output(message)?;
                }
                DialogImplBlockSelection::Quit => return Err(ProcessingError::UserCanceledDialog),
            }
        }
    }

    fn impl_block_selection(
        &self,
        impl_block: NodeIndex,
        required_node: NodeIndex,
        dialog_handler: &mut impl CgDialog<String, String>,
    ) -> ProcessingResult<DialogImplBlockSelection> {
        let impl_block_name = self.get_verbose_name_of_tree_node(impl_block)?;
        let required_node_name = self.get_verbose_name_of_tree_node(required_node)?;
        let prompt = format!(
            "Found '{}' of required '{}'.",
            impl_block_name, required_node_name
        );
        let options = vec![
            format!("Include '{}'.", impl_block_name),
            format!("Exclude '{}'.", impl_block_name),
            format!("Include all items of '{}'.", impl_block_name),
            format!("Show code of '{}'.", impl_block_name),
        ];
        if let Some(selection) = dialog_handler.select_option(
            &prompt,
            "↑↓ to move, enter to select, type to filter, and esc to quit.",
            options.clone(),
        )? {
            let user_selection =
                DialogImplBlockSelection::try_from(options.iter().position(|o| *o == selection))?;
            return Ok(user_selection);
        }
        Ok(DialogImplBlockSelection::Quit)
    }

    fn impl_block_with_trait_dialog(
        &self,
        impl_block: NodeIndex,
        required_node: NodeIndex,
        dialog_handler: &mut impl CgDialog<String, String>,
    ) -> ProcessingResult<Vec<(NodeIndex, bool)>> {
        loop {
            match self.impl_block_with_trait_selection(impl_block, required_node, dialog_handler)? {
                DialogImplBlockWithTraitSelection::IncludeImplBlock => {
                    return Ok(vec![(impl_block, true)]);
                }
                DialogImplBlockWithTraitSelection::ExcludeImplBlock => {
                    return Ok(vec![(impl_block, false)]);
                }
                DialogImplBlockWithTraitSelection::ShowImplBlock => {
                    let mut message = String::new();
                    // extracting source code span of dialog item
                    if let Some(item) = self.get_syn_item(impl_block) {
                        if let Some(src_file) = self.get_src_file_containing_item(impl_block) {
                            let span = item.span();
                            if let Some(impl_item_source) = span.source_text() {
                                writeln!(
                                    &mut message,
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
                            self.get_verbose_name_of_tree_node(impl_block)?
                        );
                    }
                    dialog_handler.write_output(message)?;
                }
                DialogImplBlockWithTraitSelection::Quit => {
                    return Err(ProcessingError::UserCanceledDialog);
                }
            }
        }
    }

    fn impl_block_with_trait_selection(
        &self,
        impl_block: NodeIndex,
        required_node: NodeIndex,
        dialog_handler: &mut impl CgDialog<String, String>,
    ) -> ProcessingResult<DialogImplBlockWithTraitSelection> {
        let impl_block_name = self.get_verbose_name_of_tree_node(impl_block)?;
        let required_node_name = self.get_verbose_name_of_tree_node(required_node)?;
        let prompt = format!(
            "Found '{}' of required '{}'.",
            impl_block_name, required_node_name
        );
        let options = vec![
            format!("Include '{}'.", impl_block_name),
            format!("Exclude '{}'.", impl_block_name),
            format!("Show code of '{}'.", impl_block_name),
        ];

        if let Some(selection) = dialog_handler.select_option(
            &prompt,
            "↑↓ to move, enter to select, type to filter, and esc to quit.",
            options.clone(),
        )? {
            let user_selection = DialogImplBlockWithTraitSelection::try_from(
                options.iter().position(|o| *o == selection),
            )?;
            return Ok(user_selection);
        }
        Ok(DialogImplBlockWithTraitSelection::Quit)
    }

    fn impl_config_toml_dialog(
        &self,
        dialog_handler: &mut impl CgDialog<String, String>,
        seen_dialog_items: &HashMap<NodeIndex, bool>,
    ) -> ProcessingResult<Option<(Utf8PathBuf, String)>> {
        let toml_config_path = self.get_impl_config_toml_path()?;
        let initial_value: String = if let Some(ref toml_path) = toml_config_path {
            toml_path.as_str().into()
        } else {
            let default_cg_fusion_config_toml = self
                .challenge_package()
                .path
                .join("./cg-fusion_config.toml");
            let current_dir = current_dir_utf8()?;
            let relative_path = get_relative_path(&current_dir, &default_cg_fusion_config_toml)?;
            relative_path.as_str().into()
        };
        // convert relative path to posix path
        let initial_value = initial_value.replace('\\', "/");
        if let Some(file_path) = dialog_handler.text_file_path(
            "Enter file path relative to crate dir to save impl config...",
            "tab to autocomplete, non existing file path will be created, esc to skip saving.",
            &initial_value,
            ConfigFilePathValidator {
                base_dir: self.challenge_package().path.to_owned(),
            },
        )? {
            // check if returning path is relative to challenge
            self.verify_path_points_inside_challenge_dir(&file_path)?;
            let full_file_path = clean_absolute_utf8(&file_path)?;
            let dir_file_path = full_file_path
                .parent()
                .context(add_context!("Expected dir of impl config toml file."))?;
            fs::create_dir_all(dir_file_path)?;
            let toml_str = if let Some(ref toml_path) = toml_config_path {
                fs::read_to_string(toml_path)?
            } else {
                IMPL_CONFIG_TOML_TEMPLATE.into()
            };
            let mut doc = toml_str.parse::<DocumentMut>()?;
            let impl_config = self.map_node_indices_to_impl_config_options(seen_dialog_items)?;
            let include_impl_items = impl_config.impl_items_include_to_toml_array();
            let exclude_impl_items = impl_config.impl_items_exclude_to_toml_array();
            let include_impl_blocks = impl_config.impl_blocks_include_to_toml_array();
            let exclude_impl_blocks = impl_config.impl_blocks_exclude_to_toml_array();
            doc["impl_items"]["include"] = value(include_impl_items);
            doc["impl_items"]["exclude"] = value(exclude_impl_items);
            doc["impl_blocks"]["include"] = value(include_impl_blocks);
            doc["impl_blocks"]["exclude"] = value(exclude_impl_blocks);

            return Ok(Some((file_path, doc.to_string())));
        }
        if self.options.verbose() {
            println!("Skipping saving impl config.");
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests;

// Checking impl items of required impl blocks. If config options do not process
// the impl item, a dialog prompts the user to decide, if the impl item should be
// include in or exclude from the challenge.

mod inquire_dialog;

use super::{FuseChallengeState, ProcessingError, ProcessingResult};
use crate::{
    add_context,
    challenge_tree::EdgeType,
    configuration::CgCliImplDialog,
    utilities::{clean_absolute_utf8, current_dir_utf8, get_relative_path, CgDialog, DialogCli},
    CgData,
};
use anyhow::Context;
use cargo_metadata::camino::Utf8PathBuf;
use inquire_dialog::{ConfigFilePathValidator, UserSelection};
use petgraph::stable_graph::NodeIndex;
use std::collections::hash_map::Entry;
use std::{
    collections::{HashMap, HashSet},
    fmt::Write as FmtWrite,
    fs,
    io::Write,
};
use syn::spanned::Spanned;
use toml_edit::{value, DocumentMut};

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

// ToDo: first check items only of challenge package (bin and lib crate)
// after this check dependencies of challenge
// ToDo: impl blocks need their own user dialog

impl<O: CgCliImplDialog> CgData<O, ProcessingImplItemDialogState> {
    pub fn check_impl_blocks(mut self) -> ProcessingResult<CgData<O, FuseChallengeState>> {
        let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();
        let impl_options = self.map_impl_config_options_to_node_indices()?;
        let mut dialog_handler = DialogCli::new(std::io::stdout());
        let mut seen_check_items: HashSet<NodeIndex> = self
            .iter_items_required_by_challenge()
            .map(|(n, _)| n)
            .collect();
        let mut got_user_input = false;
        while let Some(impl_item) = {
            let next_item_option = self
                .iter_impl_items_without_required_link_in_required_impl_blocks()
                .map(|(n, _)| n)
                .chain(
                    self.iter_impl_blocks_without_required_link_of_required_items()
                        .map(|(n, _)| n),
                )
                .find(|n| (!seen_impl_items.contains_key(n)));
            next_item_option
        } {
            let impl_block = self
                .get_parent_index_by_edge_type(impl_item, EdgeType::Syn)
                .unwrap();
            match (
                impl_options.get(&impl_item),
                self.options.processing().process_all_impl_items,
            ) {
                (Some(true), _) | (_, Some(true)) => {
                    self.add_required_by_challenge_link(impl_block, impl_item)?;
                    self.add_challenge_links_for_referenced_nodes_of_item(
                        impl_item,
                        &mut seen_check_items,
                    )?;
                    seen_impl_items.insert(impl_item, true);
                }
                (Some(false), _) | (_, Some(false)) => {
                    if self.options.verbose() {
                        println!(
                            "Excluding impl item '{}'",
                            self.get_verbose_name_of_tree_node(impl_item)?
                        );
                    }
                    seen_impl_items.insert(impl_item, false);
                }
                _ => {
                    // no  configuration for impl_item -> do user dialog
                    got_user_input = true;
                    let user_input = self.impl_item_dialog(
                        impl_item,
                        impl_block,
                        &mut dialog_handler,
                        &mut seen_impl_items,
                    )?;
                    if user_input {
                        self.add_required_by_challenge_link(impl_block, impl_item)?;
                        self.add_challenge_links_for_referenced_nodes_of_item(
                            impl_item,
                            &mut seen_check_items,
                        )?;
                    }
                    seen_impl_items.insert(impl_item, user_input);
                }
            }
        }
        // if at least once user input was required, show dialog to save impl config file.
        if got_user_input {
            if let Some((toml_path, toml_content)) =
                self.impl_config_toml_dialog(&mut dialog_handler, &seen_impl_items)?
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
        Ok(self.set_state(FuseChallengeState))
    }

    fn impl_item_dialog(
        &self,
        dialog_item: NodeIndex,
        impl_block: NodeIndex,
        dialog_handler: &mut impl CgDialog<String, String>,
        seen_impl_items: &mut HashMap<NodeIndex, bool>,
    ) -> ProcessingResult<bool> {
        loop {
            match self.impl_item_selection(dialog_item, impl_block, dialog_handler)? {
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
                            self.get_verbose_name_of_tree_node(dialog_item)?
                        );
                    }
                    dialog_handler.write_output(message)?;
                }
                UserSelection::ShowUsageOfItem => {
                    let mut message = String::new();
                    // extracting source code span of dialog item
                    for (node_index, src_span, ident) in self
                        .get_possible_usage_of_impl_item_in_required_items(dialog_item)
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
                            self.get_verbose_name_of_tree_node(dialog_item)?
                        );
                    }
                    dialog_handler.write_output(message)?;
                }
                UserSelection::Quit => return Err(ProcessingError::UserCanceledDialog),
            }
        }
    }

    fn impl_item_selection(
        &self,
        dialog_item: NodeIndex,
        impl_block: NodeIndex,
        dialog_handler: &mut impl CgDialog<String, String>,
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
        if let Some(selection) = dialog_handler.select_option(
            &prompt,
            "↑↓ to move, enter to select, type to filter, and esc to quit.",
            options.clone(),
        )? {
            let user_selection =
                UserSelection::try_from(options.iter().position(|o| *o == selection))?;
            return Ok(user_selection);
        }
        Ok(UserSelection::Quit)
    }

    fn impl_config_toml_dialog(
        &self,
        dialog_handler: &mut impl CgDialog<String, String>,
        seen_impl_items: &HashMap<NodeIndex, bool>,
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
            let impl_config = self.map_node_indices_to_impl_config_options(seen_impl_items)?;
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

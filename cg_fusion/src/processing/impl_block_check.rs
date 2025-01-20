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
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    io::Write,
};

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
            let user_input = self.impl_item_dialog(impl_item, impl_block)?;
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
    ) -> ProcessingResult<bool> {
        let mut selection_handler = SelectionCli::new(std::io::stdout());
        loop {
            let user_selection =
                self.impl_item_selection(dialog_item, impl_block, &mut selection_handler)?;
            match user_selection {
                UserSelection::IncludeItem => return Ok(true),
                UserSelection::ExcludeItem => return Ok(false),
                UserSelection::IncludeAllItemsOfImplBlock => unimplemented!(),
                UserSelection::ExcludeAllItemsOfImplBlock => unimplemented!(),
                UserSelection::ShowItem => unimplemented!(),
                UserSelection::ShowUsageOfItem => unimplemented!(),
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

struct SelectionCli<W: Write, S: Display + 'static, M: Display + 'static> {
    writer: W,
    _select_display_type: std::marker::PhantomData<S>,
    _message_display_type: std::marker::PhantomData<M>,
}

impl<W: Write> SelectionCli<W, String, String> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            _select_display_type: std::marker::PhantomData,
            _message_display_type: std::marker::PhantomData,
        }
    }
}

impl<S: Display + 'static, M: Display + 'static, W: Write> SelectionDialog<S, M>
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
mod tests {

    use std::io::stdout;

    use crate::challenge_tree::NodeType;
    use crate::parsing::ItemName;

    use super::super::tests::setup_processing_test;
    use super::*;

    use std::io::Stdout;

    // Wrapper um den Mock
    struct TestSelectionDialog<S: Display + 'static, M: Display + 'static> {
        mock: MockSelectionDialog<S, M>,
        dialog: SelectionCli<Stdout, S, M>,
    }

    impl TestSelectionDialog<String, String> {
        fn new() -> Self {
            Self {
                mock: MockSelectionDialog::<String, String>::new(),
                dialog: SelectionCli::new(stdout()),
            }
        }
    }

    impl<S: Display + 'static, M: Display + 'static> SelectionDialog<S, M>
        for TestSelectionDialog<S, M>
    {
        fn select_option(&self, prompt: &str, help: &str, options: Vec<S>) -> AnyResult<Option<S>> {
            self.mock.select_option(prompt, help, options)
        }

        // since the compiler is not able
        fn write_output(&mut self, message: M) -> AnyResult<()> {
            self.dialog.write_output(message)
        }
    }

    // ToDo: before continuing function code write some test code!
    #[test]
    fn test_select_dialog_include() {
        // preparation
        let cg_data = setup_processing_test()
            .add_challenge_dependencies()
            .unwrap()
            .add_src_files()
            .unwrap()
            .expand_use_statements()
            .unwrap()
            .link_impl_blocks_with_corresponding_item()
            .unwrap()
            .link_required_by_challenge()
            .unwrap();

        // get impl item index not required by challenge
        let set_black_index = cg_data
            .iter_crates()
            .flat_map(|(n, _, _)| cg_data.iter_syn(n))
            .find_map(|(n, nt)| match nt {
                NodeType::SynImplItem(impl_item) => if let Some(name) = ItemName::from(impl_item).get_ident_in_name_space() {
                    (name == "set_black").then_some(n)
                } else {
                    None
                }
                _ => None,
            })
            .unwrap();
        let action_impl_block_index = cg_data
                .get_parent_index_by_edge_type(set_black_index, EdgeType::Syn)
                .unwrap();
        
        // prepare mock
        let mut mock = TestSelectionDialog::new();
        mock.mock.expect_select_option()
            .with(
                eq("Found 'Action (Impl)::set_black (Impl Fn)' of required 'Action (Impl)'."),
                eq(""),
                eq(vec![
                    "Include 'Action (Impl)::set_black (Impl Fn)'.".into(),
                    "Exclude 'Action (Impl)::set_black (Impl Fn)'.".into(),
                    "Include all items of 'Action (Impl)'.".into(),
                    "Exclude all items of 'Action (Impl)'.".into(),
                    "Show code of 'Action (Impl)::set_black (Impl Fn)'.".into(),
                    "Show usage of 'Action (Impl)::set_black (Impl Fn)'.".into(),
                ]),
            )
            .returning(|_, _, _| Ok(Some("Include 'Action (Impl)::set_black (Impl Fn)'.".into())));
        // test and assert
        let test_result = cg_data.impl_item_selection(set_black_index, action_impl_block_index, &mut mock).unwrap();
        assert_eq!(test_result, UserSelection::IncludeItem)
    }
}

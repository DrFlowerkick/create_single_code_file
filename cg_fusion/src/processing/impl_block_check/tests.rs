// testing selection and dialog

use crate::{
    challenge_tree::{ChallengeTreeError, NodeType},
    configuration::FusionCli,
    parsing::ItemName,
    utilities::MockCgDialog,
};

use super::super::tests::setup_processing_test;
use super::*;

use anyhow::Result;
use cargo_metadata::camino::Utf8PathBuf;
use inquire::validator::StringValidator;
use mockall::predicate::*;
use once_cell::sync::Lazy;
use std::{fmt::Display, io::Cursor};

const PROMPT: &str = "Found 'MyMap2D (Impl)::set (Impl Fn)' of required 'MyMap2D (Impl)'.";
const HELP: &str = "↑↓ to move, enter to select, type to filter, and esc to quit.";

static OPTIONS: Lazy<Vec<String>> = Lazy::new(|| {
    vec![
        String::from("Include 'MyMap2D (Impl)::set (Impl Fn)'."),
        String::from("Exclude 'MyMap2D (Impl)::set (Impl Fn)'."),
        String::from("Include all items of 'MyMap2D (Impl)'."),
        String::from("Exclude all items of 'MyMap2D (Impl)'."),
        String::from("Show code of 'MyMap2D (Impl)::set (Impl Fn)'."),
        String::from("Show usage of 'MyMap2D (Impl)::set (Impl Fn)'."),
    ]
});

// Wrapper of Mock
struct TestCgDialog<S: Display + 'static, M: Display + 'static> {
    mock: MockCgDialog<S, M>,
    dialog: DialogCli<Cursor<Vec<u8>>, S, M>,
}

impl TestCgDialog<String, String> {
    fn new() -> Self {
        Self {
            mock: MockCgDialog::<String, String>::new(),
            dialog: DialogCli::new(Cursor::new(Vec::new())),
        }
    }
}

impl<S: Display + 'static, M: Display + 'static> CgDialog<S, M> for TestCgDialog<S, M> {
    fn select_option(&self, prompt: &str, help: &str, options: Vec<S>) -> Result<Option<S>> {
        self.mock.select_option(prompt, help, options)
    }

    fn text_file_path<V: StringValidator + 'static>(
        &self,
        prompt: &str,
        help: &str,
        initial_value: &str,
        validator: V,
    ) -> Result<Option<Utf8PathBuf>> {
        self.mock
            .text_file_path(prompt, help, initial_value, validator)
    }

    fn confirm(&self, prompt: &str, help: &str, default_value: bool) -> Result<bool> {
        self.mock.confirm(prompt, help, default_value)
    }

    fn write_output(&mut self, message: M) -> Result<()> {
        self.dialog.write_output(message)
    }
}

fn prepare_test() -> (
    CgData<FusionCli, ProcessingImplItemDialogState>,
    NodeIndex,
    NodeIndex,
) {
    // preparation
    let cg_data = setup_processing_test(false)
        .add_challenge_dependencies()
        .unwrap()
        .add_src_files()
        .unwrap()
        .expand_use_statements()
        .unwrap()
        .remove_crate_keyword_from_use_and_path_statements()
        .unwrap()
        .link_impl_blocks_with_corresponding_item()
        .unwrap()
        .link_required_by_challenge()
        .unwrap();

    // get impl item index not required by challenge
    let set_index = cg_data
        .iter_crates()
        .flat_map(|(n, _, _)| cg_data.iter_syn(n))
        .filter_map(|(n, nt)| match nt {
            NodeType::SynImplItem(impl_item) => {
                if let Some(name) = ItemName::from(impl_item).get_ident_in_name_space() {
                    (name == "set").then_some(n)
                } else {
                    None
                }
            }
            _ => None,
        })
        .find(|n| {
            let parent = cg_data
                .get_parent_index_by_edge_type(*n, EdgeType::Syn)
                .unwrap();
            let parent_name = cg_data.get_verbose_name_of_tree_node(parent).unwrap();
            parent_name == "MyMap2D (Impl)"
        })
        .unwrap();
    let my_map_2d_impl_block_index = cg_data
        .get_parent_index_by_edge_type(set_index, EdgeType::Syn)
        .unwrap();
    (cg_data, set_index, my_map_2d_impl_block_index)
}

#[test]
fn test_impl_item_selection() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

    // prepare mock for include
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[0].to_owned())));

    // prepare mock for exclude
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[1].to_owned())));

    // prepare mock for include block items
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[2].to_owned())));

    // prepare mock for exclude block items
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[3].to_owned())));

    // prepare mock for show item
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[4].to_owned())));

    // prepare mock for show usage of item
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[5].to_owned())));

    // prepare mock for use quits
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(None));

    // prepare mock for show usage of item
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Some bad output".into())));

    // test and assert
    // include
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::IncludeItem);

    // exclude
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::ExcludeItem);

    // include block items
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::IncludeAllItemsOfImplBlock);

    // exclude block items
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::ExcludeAllItemsOfImplBlock);

    // show item
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::ShowItem);

    // show usage of item
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::ShowUsageOfItem);

    // user quits
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::Quit);

    // bad output
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::Quit);
}

#[test]
fn test_impl_item_dialog_include() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

    let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();

    // prepare mock for include
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[0].to_owned())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(
            set_index,
            my_map_2d_impl_block_index,
            &mut mock,
            &mut seen_impl_items,
        )
        .unwrap();

    assert_eq!(test_result, true);
}

#[test]
fn test_impl_item_dialog_exclude() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

    let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();

    // prepare mock for exclude
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[1].to_owned())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(
            set_index,
            my_map_2d_impl_block_index,
            &mut mock,
            &mut seen_impl_items,
        )
        .unwrap();

    assert_eq!(test_result, false);
}

#[test]
fn test_impl_item_dialog_include_block_items() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

    let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();

    // prepare mock for include all block items
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[2].to_owned())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(
            set_index,
            my_map_2d_impl_block_index,
            &mut mock,
            &mut seen_impl_items,
        )
        .unwrap();

    assert_eq!(test_result, true);

    for (item_index, item) in cg_data.iter_syn_impl_item(my_map_2d_impl_block_index) {
        match ItemName::from(item)
            .get_ident_in_name_space()
            .unwrap()
            .to_string()
            .as_str()
        {
            // new is required by challenge -> will not be included in seen_impl_items
            "new" => assert_eq!(seen_impl_items.get(&item_index), None),
            // everything else is now included in seen_impl_items
            _ => assert_eq!(seen_impl_items.get(&item_index), Some(&true)),
        }
    }
}

#[test]
fn test_impl_item_dialog_exclude_block_items() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

    let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();

    // prepare mock for exclude all block items
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[3].to_owned())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(
            set_index,
            my_map_2d_impl_block_index,
            &mut mock,
            &mut seen_impl_items,
        )
        .unwrap();

    assert_eq!(test_result, false);

    for (item_index, item) in cg_data.iter_syn_impl_item(my_map_2d_impl_block_index) {
        match ItemName::from(item)
            .get_ident_in_name_space()
            .unwrap()
            .to_string()
            .as_str()
        {
            // new is required by challenge -> will not be included in seen_impl_items
            "new" => assert_eq!(seen_impl_items.get(&item_index), None),
            // everything else is now included in seen_impl_items
            _ => assert_eq!(seen_impl_items.get(&item_index), Some(&false)),
        }
    }
}

#[test]
fn test_impl_item_dialog_show_item_and_include() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

    let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();

    // prepare mock for include
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[4].to_owned())));
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[0].to_owned())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(
            set_index,
            my_map_2d_impl_block_index,
            &mut mock,
            &mut seen_impl_items,
        )
        .unwrap();

    assert_eq!(test_result, true);
    let writer_content = String::from_utf8(mock.dialog.writer.into_inner()).unwrap();
    assert_eq!(
        writer_content,
        r#"
C:\Users\User\Documents\repos\codingame\create_single_code_file\cg_fusion_lib_test\my_map_two_dim\src\lib.rs:50:5
pub fn set(&mut self, coordinates: MapPoint<X, Y>, value: T) -> &T {
        self.items[coordinates.y()][coordinates.x()] = value;
        &self.items[coordinates.y()][coordinates.x()]
    }

"#
    );
}

#[test]
fn test_impl_item_dialog_show_usage_of_item_and_exclude() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

    let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();

    // prepare mock for include
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[5].to_owned())));
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[1].to_owned())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(
            set_index,
            my_map_2d_impl_block_index,
            &mut mock,
            &mut seen_impl_items,
        )
        .unwrap();

    assert_eq!(test_result, false);
    let writer_content = String::from_utf8(mock.dialog.writer.into_inner()).unwrap();
    assert_eq!(
        writer_content,
        r#"
C:\Users\User\Documents\repos\codingame\create_single_code_file\cg_fusion_binary_test\src\lib.rs:48:20
pub fn apply_action(&mut self, action: Action) {
        self.board.set(action.cell, action.value);
    }

"#
    );
}

#[test]
fn test_impl_config_toml_dialog() {
    // preparation
    let cg_data = setup_processing_test(false)
        .add_challenge_dependencies()
        .unwrap()
        .add_src_files()
        .unwrap()
        .expand_use_statements()
        .unwrap()
        .remove_crate_keyword_from_use_and_path_statements()
        .unwrap()
        .link_impl_blocks_with_corresponding_item()
        .unwrap()
        .link_required_by_challenge()
        .unwrap();
    let base_dir = cg_data.challenge_package().path.to_owned();

    // get impl item index not required by challenge
    let set_and_get_mapping: HashMap<NodeIndex, bool> = cg_data
        .iter_crates()
        .flat_map(|(n, _, _)| cg_data.iter_syn(n))
        .filter_map(|(n, nt)| match nt {
            NodeType::SynImplItem(impl_item) => {
                if let Some(name) = ItemName::from(impl_item).get_ident_in_name_space() {
                    (name == "get" || name == "set").then_some(n)
                } else {
                    None
                }
            }
            _ => None,
        })
        .map(|n| {
            let impl_block_index = cg_data
                .get_parent_index_by_edge_type(n, EdgeType::Syn)
                .unwrap();
            let item_node = cg_data
                .get_parent_index_by_edge_type(impl_block_index, EdgeType::Implementation)
                .unwrap();
            let item = cg_data.get_syn_item(item_node).unwrap();
            let item_ident = ItemName::from(item).get_ident_in_name_space().unwrap();
            (n, item_ident == "MyMap2D")
        })
        .collect();

    // prepare mock for include
    let mut mock = TestCgDialog::new();
    let validator = ConfigFilePathValidator { base_dir };
    // returning valid path
    mock.mock
        .expect_text_file_path()
        .times(1)
        .with(
            eq("Enter file path relative to crate dir to save impl config..."),
            eq("tab to autocomplete, non existing file path will be created, esc to skip saving."),
            eq("../cg_fusion_binary_test/cg-fusion_config.toml"),
            eq(validator.clone()),
        )
        .return_once(|_, _, _, _| {
            Ok(Some(Utf8PathBuf::from(
                "../cg_fusion_binary_test/cg-fusion_config.toml",
            )))
        });
    // returning invalid path
    mock.mock
        .expect_text_file_path()
        .times(1)
        .with(
            eq("Enter file path relative to crate dir to save impl config..."),
            eq("tab to autocomplete, non existing file path will be created, esc to skip saving."),
            eq("../cg_fusion_binary_test/cg-fusion_config.toml"),
            eq(validator.clone()),
        )
        .return_once(|_, _, _, _| Ok(Some(Utf8PathBuf::from("./cg-fusion_config.toml"))));
    // skipping saving of toml file
    mock.mock
        .expect_text_file_path()
        .times(1)
        .with(
            eq("Enter file path relative to crate dir to save impl config..."),
            eq("tab to autocomplete, non existing file path will be created, esc to skip saving."),
            eq("../cg_fusion_binary_test/cg-fusion_config.toml"),
            eq(validator),
        )
        .return_once(|_, _, _, _| Ok(None));

    // assert
    // returning new toml file path and content
    let (toml_path, toml_content) = cg_data
        .impl_config_toml_dialog(&mut mock, &set_and_get_mapping)
        .unwrap()
        .unwrap();
    assert_eq!(
        toml_path,
        Utf8PathBuf::from("../cg_fusion_binary_test/cg-fusion_config.toml")
    );
    assert_eq!(
        toml_content,
        r#"# impl config file in TOML format to configure included or excluded impl items of
# specific user defined types in respectively from challenge.
# file structure:
# include_impl_items = [include_item_1, include_item_2]
# exclude_impl_items = [exclude_item_1, exclude_item_2]
#
# If the name of the impl item is ambiguous (e.g. push(), next(), etc.), add as much
# information to the name as is required to make the name unique including the name of
# the user defined type:
# path::to::module::of::impl_block_of_user_defined_type_name::user_defined_type_name::impl_item_name.
#
# Usage of wildcard '*' for impl item is possible, if at least the name of the user defined type is
# given. E.g. 'user_defined_type_name::*' will include or exclude all impl items of
# 'user_defined_type_name'.
#
# If in conflict with other impl options, the 'include' option always wins.
include_impl_items = ["MyMap2D::get", "MyMap2D::set"]
exclude_impl_items = ["MyArray::get", "MyArray::set"]
"#
    );

    // returning error because of invalid path
    let test_result = cg_data.impl_config_toml_dialog(&mut mock, &set_and_get_mapping);
    assert!(matches!(
        test_result,
        Err(ProcessingError::ChallengeTreeError(
            ChallengeTreeError::NotInsideChallengeDir(_)
        ))
    ));

    // returning Ok(None) if user skips file path dialog
    let test_result = cg_data
        .impl_config_toml_dialog(&mut mock, &set_and_get_mapping)
        .unwrap();
    assert!(test_result.is_none());
}

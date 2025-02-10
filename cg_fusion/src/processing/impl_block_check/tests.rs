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

const PROMPT: &str = "Found 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set (Impl Fn)' of required 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>'.";
const HELP: &str = "↑↓ to move, enter to select, type to filter, and esc to quit.";

static OPTIONS: Lazy<Vec<String>> = Lazy::new(|| {
    vec![
        String::from("Include 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set (Impl Fn)'."),
        String::from("Exclude 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set (Impl Fn)'."),
        String::from("Include all items of 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>'."),
        String::from("Exclude all items of 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>'."),
        String::from("Show code of 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set (Impl Fn)'."),
        String::from("Show usage of 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set (Impl Fn)'."),
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
        .path_minimizing_of_use_and_path_statements()
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
            parent_name == "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>"
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
fn test_list_order_required_modules_and_crates() {
    // preparation
    let cg_data = setup_processing_test(false)
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
        .unwrap();

    let list = cg_data
        .get_required_crates_and_modules_sorted_by_relevance()
        .unwrap();
    for node in list {
        println!("{}", cg_data.get_verbose_name_of_tree_node(node).unwrap());
    }
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
        .path_minimizing_of_use_and_path_statements()
        .unwrap()
        .link_impl_blocks_with_corresponding_item()
        .unwrap()
        .link_required_by_challenge()
        .unwrap();
    let base_dir = cg_data.challenge_package().path.to_owned();

    // get impl item index not required by challenge
    let impl_config_mapping: HashMap<NodeIndex, bool> = cg_data
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
            NodeType::SynItem(item) => {
                if let ItemName::ImplBlockIdentifier(name) = ItemName::from(item) {
                    (name.contains("Default for") && name.contains("T:")).then_some(n)
                } else {
                    None
                }
            }
            _ => None,
        })
        .map(|n| {
            if cg_data.is_syn_impl_item(n) {
                let impl_block_index = cg_data
                    .get_parent_index_by_edge_type(n, EdgeType::Syn)
                    .unwrap();
                let item_node = cg_data
                    .get_parent_index_by_edge_type(impl_block_index, EdgeType::Implementation)
                    .unwrap();
                let item = cg_data.get_syn_item(item_node).unwrap();
                let item_ident = ItemName::from(item).get_ident_in_name_space().unwrap();
                (n, item_ident == "MyMap2D")
            } else {
                let item = cg_data.get_syn_item(n).unwrap();
                let ItemName::ImplBlockIdentifier(name) = ItemName::from(item) else {
                    panic!("Expected name of impl block");
                };
                (n, name.ends_with("MyMap2D<T,X,Y,N>"))
            }
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
        .impl_config_toml_dialog(&mut mock, &impl_config_mapping)
        .unwrap()
        .unwrap();
    assert_eq!(
        toml_path,
        Utf8PathBuf::from("../cg_fusion_binary_test/cg-fusion_config.toml")
    );
    assert_eq!(
        toml_content,
        r#"# impl config file in TOML format to configure impl items of specific impl blocks to
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
include = [
    "get@impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>",
    "set@impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>"
]
exclude = [
    "get@impl<T:Copy+Clone+Default,constN:usize> MyArray<T,N>",
    "set@impl<T:Copy+Clone+Default,constN:usize> MyArray<T,N>"
]
[impl_blocks]
include = [
    "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>"
]
exclude = [
    "impl<T:Copy+Clone+Default,constN:usize> Default for MyArray<T,N>"
]
"#
    );

    // returning error because of invalid path
    let test_result = cg_data.impl_config_toml_dialog(&mut mock, &impl_config_mapping);
    assert!(matches!(
        test_result,
        Err(ProcessingError::ChallengeTreeError(
            ChallengeTreeError::NotInsideChallengeDir(_)
        ))
    ));

    // returning Ok(None) if user skips file path dialog
    let test_result = cg_data
        .impl_config_toml_dialog(&mut mock, &impl_config_mapping)
        .unwrap();
    assert!(test_result.is_none());
}

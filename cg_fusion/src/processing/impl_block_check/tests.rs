// testing selection and dialog

use crate::{
    challenge_tree::{ChallengeTreeError, EdgeType, NodeType},
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

#[cfg(test)]
mod impl_item;

#[cfg(test)]
mod impl_block;

#[cfg(test)]
mod impl_block_with_trait;

// ToDo: delete this later. Just keep it for further debugging
use crate::{CgDataBuilder, CgMode, ProcessingDependenciesState, configuration::CargoCli};
use petgraph::{Direction, visit::EdgeRef};

fn setup_processing_test_ult_tic_tac_toe(
    impl_config: bool,
) -> CgData<FusionCli, ProcessingDependenciesState> {
    let mut fusion_options = FusionCli::default();
    fusion_options.set_manifest_path("../../cg_ultimate_tic_tac_toe/Cargo.toml".into());
    if impl_config {
        fusion_options
            .set_impl_item_toml("../../cg_ultimate_tic_tac_toe/cg-fusion_config.toml".into());
    }

    let cg_data = match CgDataBuilder::new()
        .set_options(CargoCli::CgFusion(fusion_options))
        .set_command()
        .build()
        .unwrap()
    {
        CgMode::Fusion(cg_data) => cg_data,
    };
    cg_data
}
#[test]
fn test_whats_up_with_tictactoe_status() {
    // preparation
    let cg_data = setup_processing_test_ult_tic_tac_toe(false)
        .add_challenge_dependencies()
        .unwrap()
        .add_src_files()
        .unwrap()
        .expand_use_statements()
        .unwrap()
        .expand_external_use_statements()
        .unwrap()
        .path_minimizing_of_use_and_path_statements()
        .unwrap()
        .link_impl_blocks_with_corresponding_item()
        .unwrap()
        .link_required_by_challenge()
        .unwrap();

    let enum_tictactoe_status_node = cg_data
        .iter_crates()
        .flat_map(|(n, _, _)| cg_data.iter_syn(n))
        .find_map(|(n, i)| {
            if let NodeType::SynItem(Item::Enum(item_enum)) = i {
                (item_enum.ident == "TicTacToeStatus").then_some(n)
            } else {
                None
            }
        })
        .unwrap();

    println!(
        "linked nodes for {}",
        cg_data
            .get_verbose_name_of_tree_node(enum_tictactoe_status_node)
            .unwrap()
    );
    for (linked_node, edge_type, direction) in cg_data
        .tree
        .edges_directed(enum_tictactoe_status_node, Direction::Incoming)
        .map(|e| (e.source(), e.weight(), Direction::Incoming))
        .chain(
            cg_data
                .tree
                .edges_directed(enum_tictactoe_status_node, Direction::Outgoing)
                .map(|e| (e.target(), e.weight(), Direction::Outgoing)),
        )
    {
        println!(
            "'{}', et: {:?}, dir: {:?}",
            cg_data.get_verbose_name_of_tree_node(linked_node).unwrap(),
            edge_type,
            direction
        );
    }

    let impl_tictactoe_status_node = cg_data
        .iter_crates()
        .flat_map(|(n, _, _)| cg_data.iter_syn(n))
        .find_map(|(n, i)| {
            if let NodeType::SynItem(item) = i {
                if let ItemName::ImplBlockIdentifier(name) = ItemName::from(item) {
                    (name == "impl TicTacToeStatus").then_some(n)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();

    println!(
        "linked nodes for {}",
        cg_data
            .get_verbose_name_of_tree_node(impl_tictactoe_status_node)
            .unwrap()
    );
    for (linked_node, edge_type, direction) in cg_data
        .tree
        .edges_directed(impl_tictactoe_status_node, Direction::Incoming)
        .map(|e| (e.source(), e.weight(), Direction::Incoming))
        .chain(
            cg_data
                .tree
                .edges_directed(impl_tictactoe_status_node, Direction::Outgoing)
                .map(|e| (e.target(), e.weight(), Direction::Outgoing)),
        )
    {
        println!(
            "'{}', et: {:?}, dir: {:?}",
            cg_data.get_verbose_name_of_tree_node(linked_node).unwrap(),
            edge_type,
            direction
        );
    }

    for (impl_item, _) in cg_data.iter_syn_impl_item(impl_tictactoe_status_node) {
        println!(
            "linked nodes for {}",
            cg_data.get_verbose_name_of_tree_node(impl_item).unwrap()
        );
        for (linked_node, edge_type, direction) in cg_data
            .tree
            .edges_directed(impl_item, Direction::Incoming)
            .map(|e| (e.source(), e.weight(), Direction::Incoming))
            .chain(
                cg_data
                    .tree
                    .edges_directed(impl_item, Direction::Outgoing)
                    .map(|e| (e.target(), e.weight(), Direction::Outgoing)),
            )
        {
            println!(
                "'{}', et: {:?}, dir: {:?}",
                cg_data.get_verbose_name_of_tree_node(linked_node).unwrap(),
                edge_type,
                direction
            );
        }
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
        .expand_external_use_statements()
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

// tests for dialog of impl item

use super::*;

const PROMPT: &str = "Found 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set (Impl Fn)' of required 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>'.";
const HELP: &str = "↑↓ to move, enter to select, type to filter, and esc to quit.";

static OPTIONS: Lazy<Vec<String>> = Lazy::new(|| {
    vec![
        String::from(
            "Include 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set (Impl Fn)'.",
        ),
        String::from(
            "Exclude 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set (Impl Fn)'.",
        ),
        String::from(
            "Include all items of 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>'.",
        ),
        String::from(
            "Exclude all items of 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>'.",
        ),
        String::from(
            "Show code of 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set (Impl Fn)'.",
        ),
        String::from(
            "Show usage of 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set (Impl Fn)'.",
        ),
    ]
});

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
    assert_eq!(test_result, DialogImplItemSelection::IncludeItem);

    // exclude
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, DialogImplItemSelection::ExcludeItem);

    // include block items
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(
        test_result,
        DialogImplItemSelection::IncludeAllItemsOfImplBlock
    );

    // exclude block items
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(
        test_result,
        DialogImplItemSelection::ExcludeAllItemsOfImplBlock
    );

    // show item
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, DialogImplItemSelection::ShowItem);

    // show usage of item
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, DialogImplItemSelection::ShowUsageOfItem);

    // user quits
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, DialogImplItemSelection::Quit);

    // bad output
    let test_result = cg_data
        .impl_item_selection(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, DialogImplItemSelection::Quit);
}

#[test]
fn test_impl_item_dialog_include() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

    // prepare mock for include
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[0].to_owned())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();

    assert_eq!(test_result, vec![(set_index, true)]);
}

#[test]
fn test_impl_item_dialog_exclude() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

    // prepare mock for exclude
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[1].to_owned())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();

    assert_eq!(test_result, vec![(set_index, false)]);
}

#[test]
fn test_impl_item_dialog_include_block_items() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

    // prepare mock for include all block items
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[2].to_owned())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();

    let expected_result: Vec<(NodeIndex, bool)> = cg_data
        .iter_syn_impl_item(my_map_2d_impl_block_index)
        .filter_map(|(n, _)| (!cg_data.is_required_by_challenge(n)).then_some((n, true)))
        .collect();

    assert_eq!(test_result, expected_result);
}

#[test]
fn test_impl_item_dialog_exclude_block_items() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

    // prepare mock for exclude all block items
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[3].to_owned())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();

    let expected_result: Vec<(NodeIndex, bool)> = cg_data
        .iter_syn_impl_item(my_map_2d_impl_block_index)
        .filter_map(|(n, _)| (!cg_data.is_required_by_challenge(n)).then_some((n, false)))
        .collect();

    assert_eq!(test_result, expected_result);
}

#[test]
fn test_impl_item_dialog_show_item_and_include() {
    // preparation
    let (cg_data, set_index, my_map_2d_impl_block_index) = prepare_test();

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
        .impl_item_dialog(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();

    assert_eq!(test_result, vec![(set_index, true)]);
    let writer_content = String::from_utf8(mock.dialog.writer.into_inner()).unwrap();
    assert_eq!(
        writer_content,
        r#"
/home/marc/Development/repos/codingame/create_single_code_file/cg_fusion_lib_test/my_map_two_dim/src/lib.rs:50:5
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
        .impl_item_dialog(set_index, my_map_2d_impl_block_index, &mut mock)
        .unwrap();

    assert_eq!(test_result, vec![(set_index, false)]);
    let writer_content = String::from_utf8(mock.dialog.writer.into_inner()).unwrap();
    assert_eq!(
        writer_content,
        r#"
/home/marc/Development/repos/codingame/create_single_code_file/cg_fusion_binary_test/src/lib.rs:39:20
pub fn apply_action(&mut self, action: Action) {
        self.board.set(action.cell, action.value);
    }

"#
    );
}

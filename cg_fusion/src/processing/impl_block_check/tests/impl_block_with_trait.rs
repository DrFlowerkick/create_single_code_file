// tests for dialog of impl item

use super::*;

const PROMPT: &str = "Found 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>' of required 'MyMap2D (Struct)'.";
const HELP: &str = "↑↓ to move, enter to select, type to filter, and esc to quit.";

static OPTIONS: Lazy<Vec<String>> = Lazy::new(|| {
    vec![
        String::from(
            "Include 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>'.",
        ),
        String::from(
            "Exclude 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>'.",
        ),
        String::from(
            "Show code of 'impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>'.",
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

    // get impl block index
    let my_map_2d_impl_default_block_index = cg_data
        .iter_crates()
        .flat_map(|(n, _, _)| cg_data.iter_syn(n).map(|(n, _)| n))
        .find(|n| {
            let block_name = cg_data.get_verbose_name_of_tree_node(*n).unwrap();
            block_name == "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>"
        })
        .unwrap();
    let struct_my_map_2d_node = cg_data
        .iter_crates()
        .flat_map(|(n, _, _)| cg_data.iter_syn_items(n))
        .filter(|(_, i)| matches!(i, Item::Struct(_)))
        .find_map(|(n, i)| {
            if let Some(name) = ItemName::from(i).get_ident_in_name_space() {
                (name == "MyMap2D").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    (
        cg_data,
        my_map_2d_impl_default_block_index,
        struct_my_map_2d_node,
    )
}

#[test]
fn test_impl_block_with_trait_selection() {
    // preparation
    let (cg_data, my_map_2d_impl_default_block_index, struct_my_map_2d_node) = prepare_test();

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
        .impl_block_with_trait_selection(
            my_map_2d_impl_default_block_index,
            struct_my_map_2d_node,
            &mut mock,
        )
        .unwrap();
    assert_eq!(
        test_result,
        DialogImplBlockWithTraitSelection::IncludeImplBlock
    );

    // exclude
    let test_result = cg_data
        .impl_block_with_trait_selection(
            my_map_2d_impl_default_block_index,
            struct_my_map_2d_node,
            &mut mock,
        )
        .unwrap();
    assert_eq!(
        test_result,
        DialogImplBlockWithTraitSelection::ExcludeImplBlock
    );

    // exclude block items
    let test_result = cg_data
        .impl_block_with_trait_selection(
            my_map_2d_impl_default_block_index,
            struct_my_map_2d_node,
            &mut mock,
        )
        .unwrap();
    assert_eq!(
        test_result,
        DialogImplBlockWithTraitSelection::ShowImplBlock
    );

    // user quits
    let test_result = cg_data
        .impl_block_with_trait_selection(
            my_map_2d_impl_default_block_index,
            struct_my_map_2d_node,
            &mut mock,
        )
        .unwrap();
    assert_eq!(test_result, DialogImplBlockWithTraitSelection::Quit);

    // bad output
    let test_result = cg_data
        .impl_block_with_trait_selection(
            my_map_2d_impl_default_block_index,
            struct_my_map_2d_node,
            &mut mock,
        )
        .unwrap();
    assert_eq!(test_result, DialogImplBlockWithTraitSelection::Quit);
}

#[test]
fn test_impl_block_with_trait_dialog_include() {
    // preparation
    let (cg_data, my_map_2d_impl_default_block_index, struct_my_map_2d_node) = prepare_test();

    // prepare mock for include
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[0].to_owned())));

    // assert
    let test_result = cg_data
        .impl_block_with_trait_dialog(
            my_map_2d_impl_default_block_index,
            struct_my_map_2d_node,
            &mut mock,
        )
        .unwrap();

    assert_eq!(
        test_result,
        vec![(my_map_2d_impl_default_block_index, true)]
    );
}

#[test]
fn test_impl_block_with_trait_dialog_exclude() {
    // preparation
    let (cg_data, my_map_2d_impl_default_block_index, struct_my_map_2d_node) = prepare_test();

    // prepare mock for exclude
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[1].to_owned())));

    // assert
    let test_result = cg_data
        .impl_block_with_trait_dialog(
            my_map_2d_impl_default_block_index,
            struct_my_map_2d_node,
            &mut mock,
        )
        .unwrap();

    assert_eq!(
        test_result,
        vec![(my_map_2d_impl_default_block_index, false)]
    );
}

#[test]
fn test_impl_block_with_trait_dialog_show_block_and_include() {
    // preparation
    let (cg_data, my_map_2d_impl_default_block_index, struct_my_map_2d_node) = prepare_test();

    // prepare mock for include
    let mut mock = TestCgDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[2].to_owned())));
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(HELP), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some(OPTIONS[0].to_owned())));

    // assert
    let test_result = cg_data
        .impl_block_with_trait_dialog(
            my_map_2d_impl_default_block_index,
            struct_my_map_2d_node,
            &mut mock,
        )
        .unwrap();

    assert_eq!(
        test_result,
        vec![(my_map_2d_impl_default_block_index, true)]
    );
    let writer_content = String::from_utf8(mock.dialog.writer.into_inner()).unwrap();
    assert_eq!(
        writer_content,
        r#"
/home/marc/Development/repos/codingame/create_single_code_file/cg_fusion_lib_test/my_map_two_dim/src/lib.rs:207:1
impl<T: Copy + Clone + Default, const X: usize, const Y: usize, const N: usize> Default
    for MyMap2D<T, X, Y, N>
{
    fn default() -> Self {
        Self::new()
    }
}

"#
    );
}

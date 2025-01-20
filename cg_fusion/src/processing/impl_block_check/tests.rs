// testing selection and dialog

use crate::challenge_tree::NodeType;
use crate::configuration::FusionCli;
use crate::parsing::ItemName;

use super::super::tests::setup_processing_test;
use super::*;

use once_cell::sync::Lazy;
use std::io::Cursor;

const PROMPT: &str = "Found 'Action (Impl)::set_black (Impl Fn)' of required 'Action (Impl)'.";

static OPTIONS: Lazy<Vec<String>> = Lazy::new(|| {
    vec![
        String::from("Include 'Action (Impl)::set_black (Impl Fn)'."),
        String::from("Exclude 'Action (Impl)::set_black (Impl Fn)'."),
        String::from("Include all items of 'Action (Impl)'."),
        String::from("Exclude all items of 'Action (Impl)'."),
        String::from("Show code of 'Action (Impl)::set_black (Impl Fn)'."),
        String::from("Show usage of 'Action (Impl)::set_black (Impl Fn)'."),
    ]
});

// Wrapper of Mock
struct TestSelectionDialog<S: Display + 'static, M: Display + 'static> {
    mock: MockSelectionDialog<S, M>,
    dialog: SelectionCli<Cursor<Vec<u8>>, S, M>,
}

impl TestSelectionDialog<String, String> {
    fn new() -> Self {
        Self {
            mock: MockSelectionDialog::<String, String>::new(),
            dialog: SelectionCli::new(Cursor::new(Vec::new())),
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

fn prepare_test() -> (
    CgData<FusionCli, ProcessingImplItemDialogState>,
    NodeIndex,
    NodeIndex,
) {
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
            NodeType::SynImplItem(impl_item) => {
                if let Some(name) = ItemName::from(impl_item).get_ident_in_name_space() {
                    (name == "set_black").then_some(n)
                } else {
                    None
                }
            }
            _ => None,
        })
        .unwrap();
    let action_impl_block_index = cg_data
        .get_parent_index_by_edge_type(set_black_index, EdgeType::Syn)
        .unwrap();
    (cg_data, set_black_index, action_impl_block_index)
}

#[test]
fn test_impl_item_selection() {
    // preparation
    let (cg_data, set_black_index, action_impl_block_index) = prepare_test();

    // prepare mock for include
    let mut mock = TestSelectionDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Include 'Action (Impl)::set_black (Impl Fn)'.".into())));

    // prepare mock for exclude
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Exclude 'Action (Impl)::set_black (Impl Fn)'.".into())));

    // prepare mock for include block items
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Include all items of 'Action (Impl)'.".into())));

    // prepare mock for exclude block items
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Exclude all items of 'Action (Impl)'.".into())));

    // prepare mock for show item
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| {
            Ok(Some(
                "Show code of 'Action (Impl)::set_black (Impl Fn)'.".into(),
            ))
        });

    // prepare mock for show usage of item
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| {
            Ok(Some(
                "Show usage of 'Action (Impl)::set_black (Impl Fn)'.".into(),
            ))
        });

    // prepare mock for use quits
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(None));

    // test and assert
    // include
    let test_result = cg_data
        .impl_item_selection(set_black_index, action_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::IncludeItem);

    // exclude
    let test_result = cg_data
        .impl_item_selection(set_black_index, action_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::ExcludeItem);

    // include block items
    let test_result = cg_data
        .impl_item_selection(set_black_index, action_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::IncludeAllItemsOfImplBlock);

    // exclude block items
    let test_result = cg_data
        .impl_item_selection(set_black_index, action_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::ExcludeAllItemsOfImplBlock);

    // show item
    let test_result = cg_data
        .impl_item_selection(set_black_index, action_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::ShowItem);

    // show usage of item
    let test_result = cg_data
        .impl_item_selection(set_black_index, action_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::ShowUsageOfItem);

    // user quits
    let test_result = cg_data
        .impl_item_selection(set_black_index, action_impl_block_index, &mut mock)
        .unwrap();
    assert_eq!(test_result, UserSelection::Quit);
}

#[test]
fn test_impl_item_dialog_include() {
    // preparation
    let (cg_data, set_black_index, action_impl_block_index) = prepare_test();

    let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();

    // prepare mock for include
    let mut mock = TestSelectionDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Include 'Action (Impl)::set_black (Impl Fn)'.".into())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(
            set_black_index,
            action_impl_block_index,
            &mut mock,
            &mut seen_impl_items,
        )
        .unwrap();

    assert_eq!(test_result, true);
}

#[test]
fn test_impl_item_dialog_exclude() {
    // preparation
    let (cg_data, set_black_index, action_impl_block_index) = prepare_test();

    let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();

    // prepare mock for exclude
    let mut mock = TestSelectionDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Exclude 'Action (Impl)::set_black (Impl Fn)'.".into())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(
            set_black_index,
            action_impl_block_index,
            &mut mock,
            &mut seen_impl_items,
        )
        .unwrap();

    assert_eq!(test_result, false);
}

#[test]
fn test_impl_item_dialog_include_block_items() {
    // preparation
    let (cg_data, set_black_index, action_impl_block_index) = prepare_test();

    let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();

    // prepare mock for include all block items
    let mut mock = TestSelectionDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Include all items of 'Action (Impl)'.".into())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(
            set_black_index,
            action_impl_block_index,
            &mut mock,
            &mut seen_impl_items,
        )
        .unwrap();

    assert_eq!(test_result, true);

    for (item_index, item) in cg_data.iter_syn_impl_item(action_impl_block_index) {
        match ItemName::from(item).get_ident_in_name_space().unwrap().to_string().as_str() {
            "set_white" => assert_eq!(seen_impl_items.get(&item_index), None),
            "set_black" => assert_eq!(seen_impl_items.get(&item_index), Some(&true)),
            _ => unimplemented!("Test must be expanded if test lib is expanded.")
        }
        
    }
}

#[test]
fn test_impl_item_dialog_exclude_block_items() {
    // preparation
    let (cg_data, set_black_index, action_impl_block_index) = prepare_test();

    let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();

    // prepare mock for exclude all block items
    let mut mock = TestSelectionDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Exclude all items of 'Action (Impl)'.".into())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(
            set_black_index,
            action_impl_block_index,
            &mut mock,
            &mut seen_impl_items,
        )
        .unwrap();

    assert_eq!(test_result, false);

    for (item_index, item) in cg_data.iter_syn_impl_item(action_impl_block_index) {
        match ItemName::from(item).get_ident_in_name_space().unwrap().to_string().as_str() {
            "set_white" => assert_eq!(seen_impl_items.get(&item_index), None),
            "set_black" => assert_eq!(seen_impl_items.get(&item_index), Some(&false)),
            _ => unimplemented!("Test must be expanded if test lib is expanded.")
        }
        
    }
}

#[test]
fn test_impl_item_dialog_show_item_and_include() {
    // preparation
    let (cg_data, set_black_index, action_impl_block_index) = prepare_test();

    let mut seen_impl_items: HashMap<NodeIndex, bool> = HashMap::new();

    // prepare mock for include
    let mut mock = TestSelectionDialog::new();
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Show code of 'Action (Impl)::set_black (Impl Fn)'.".into())));
    mock.mock
        .expect_select_option()
        .times(1)
        .with(eq(PROMPT), eq(""), eq(OPTIONS.to_owned()))
        .return_once(|_, _, _| Ok(Some("Include 'Action (Impl)::set_black (Impl Fn)'.".into())));

    // assert
    let test_result = cg_data
        .impl_item_dialog(
            set_black_index,
            action_impl_block_index,
            &mut mock,
            &mut seen_impl_items,
        )
        .unwrap();

    assert_eq!(test_result, true);
    let writer_content = String::from_utf8(mock.dialog.writer.into_inner()).unwrap();
    dbg!(writer_content);
}

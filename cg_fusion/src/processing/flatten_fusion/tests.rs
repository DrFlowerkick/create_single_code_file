// contains all tests of flatten_fusion

use quote::ToTokens;
use syn::Item;

use crate::parsing::ItemName;
use crate::processing::flatten_fusion::FlattenAgent;

use super::super::tests::setup_processing_test;
use super::*;

#[test]
fn test_set_parent() {
    // preparation
    let cg_data = setup_processing_test(true)
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
        .unwrap()
        .check_impl_blocks()
        .unwrap()
        .process_external_dependencies()
        .unwrap()
        .fuse_challenge()
        .unwrap();

    let (fusion_node, _) = cg_data.get_fusion_bin_crate().unwrap();

    // test mod MapPoint
    let map_point_mod = cg_data
        .iter_syn_items(fusion_node)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "my_map_point").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    // fusion of my_map_point does not contain further mod
    assert!(
        !cg_data
            .iter_syn_item_neighbors(map_point_mod)
            .any(|(_, i)| matches!(i, Item::Mod(_)))
    );
    let mut flatten_agent = FlattenAgent::new(map_point_mod);

    // action to test
    flatten_agent.set_parent(&cg_data);

    assert_eq!(
        cg_data
            .get_verbose_name_of_tree_node(flatten_agent.parent)
            .unwrap(),
        "my_map_two_dim (Mod)"
    );

    let items: Vec<String> = flatten_agent
        .parent_items
        .iter()
        .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
        .collect();
    assert_eq!(
        items,
        [
            "my_map_point (Mod)",
            "MyMap2D (Struct)",
            "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>",
            "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>"
        ]
    );

    let use_of_flatten: Vec<String> = flatten_agent
        .parent_use_of_flatten
        .iter()
        .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
        .collect();
    assert_eq!(use_of_flatten, ["MapPoint (Use)"]);

    assert!(flatten_agent.parent_use_of_external.is_empty());

    // test mod Action
    let action_mod = cg_data
        .iter_syn_items(fusion_node)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "action").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    // fusion of my_map_point does not contain further mod
    assert!(
        !cg_data
            .iter_syn_item_neighbors(action_mod)
            .any(|(_, i)| matches!(i, Item::Mod(_)))
    );
    let mut flatten_agent = FlattenAgent::new(action_mod);

    // action to test
    flatten_agent.set_parent(&cg_data);

    assert_eq!(
        cg_data
            .get_verbose_name_of_tree_node(flatten_agent.parent)
            .unwrap(),
        "cg_fusion_binary_test (Mod)"
    );

    let items: Vec<String> = flatten_agent
        .parent_items
        .iter()
        .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
        .collect();
    assert_eq!(
        items,
        [
            "action (Mod)",
            "fmt (Use)",
            "X (Const)",
            "Y (Const)",
            "N (Const)",
            "Value (Enum)",
            "impl fmt::Display for Value",
            "Go (Struct)",
            "impl Default for Go",
            "impl Go",
            "MyMap2D (Use)"
        ]
    );

    let use_of_flatten: Vec<String> = flatten_agent
        .parent_use_of_flatten
        .iter()
        .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
        .collect();
    assert_eq!(use_of_flatten, ["Action (Use)"]);

    assert_eq!(flatten_agent.parent_use_of_external.len(), 1);
    assert!(matches!(
        flatten_agent.parent_use_of_external[0],
        PathElement::ExternalItem(_)
    ));
    if let PathElement::ExternalItem(ref item) = flatten_agent.parent_use_of_external[0] {
        assert!(item == "fmt");
    }
}

#[test]
fn test_set_flatten_items() {
    // preparation
    let cg_data = setup_processing_test(true)
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
        .unwrap()
        .check_impl_blocks()
        .unwrap()
        .process_external_dependencies()
        .unwrap()
        .fuse_challenge()
        .unwrap();

    let (fusion_node, _) = cg_data.get_fusion_bin_crate().unwrap();

    // test mod MapPoint
    let map_point_mod = cg_data
        .iter_syn_items(fusion_node)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "my_map_point").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    // fusion of my_map_point does not contain further mod
    assert!(
        !cg_data
            .iter_syn_item_neighbors(map_point_mod)
            .any(|(_, i)| matches!(i, Item::Mod(_)))
    );
    let mut flatten_agent = FlattenAgent::new(map_point_mod);
    flatten_agent.set_parent(&cg_data);

    // action to test
    flatten_agent.set_flatten_items(&cg_data);

    let flatten_items: Vec<String> = flatten_agent
        .flatten_items
        .iter()
        .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
        .collect();

    assert_eq!(
        flatten_items,
        [
            "MapPoint (Struct)",
            "impl<constX:usize,constY:usize> MapPoint<X,Y>",
        ]
    );

    // test mod Action
    let action_mod = cg_data
        .iter_syn_items(fusion_node)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "action").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    // fusion of my_map_point does not contain further mod
    assert!(
        !cg_data
            .iter_syn_item_neighbors(action_mod)
            .any(|(_, i)| matches!(i, Item::Mod(_)))
    );
    let mut flatten_agent = FlattenAgent::new(action_mod);
    flatten_agent.set_parent(&cg_data);

    // action to test
    flatten_agent.set_flatten_items(&cg_data);

    let flatten_items: Vec<String> = flatten_agent
        .flatten_items
        .iter()
        .filter_map(|n| cg_data.get_verbose_name_of_tree_node(*n).ok())
        .collect();

    assert_eq!(
        flatten_items,
        [
            "Display (Use)",
            "MapPoint (Use)",
            "Action (Struct)",
            "impl Display for Action",
            "impl Action"
        ]
    );
}

#[test]
fn test_is_name_space_conflict() {
    // preparation
    let cg_data = setup_processing_test(true)
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
        .unwrap()
        .check_impl_blocks()
        .unwrap()
        .process_external_dependencies()
        .unwrap()
        .fuse_challenge()
        .unwrap();

    let (fusion_node, _) = cg_data.get_fusion_bin_crate().unwrap();

    // test mod MapPoint
    let map_point_mod = cg_data
        .iter_syn_items(fusion_node)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "my_map_point").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    // fusion of my_map_point does not contain further mod
    assert!(
        !cg_data
            .iter_syn_item_neighbors(map_point_mod)
            .any(|(_, i)| matches!(i, Item::Mod(_)))
    );
    let mut flatten_agent = FlattenAgent::new(map_point_mod);
    flatten_agent.set_parent(&cg_data);
    flatten_agent.set_flatten_items(&cg_data);

    // action to test
    assert!(!flatten_agent.is_name_space_conflict(&cg_data));

    // test mod Action
    let action_mod = cg_data
        .iter_syn_items(fusion_node)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "action").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    // fusion of my_map_point does not contain further mod
    assert!(
        !cg_data
            .iter_syn_item_neighbors(action_mod)
            .any(|(_, i)| matches!(i, Item::Mod(_)))
    );
    let mut flatten_agent = FlattenAgent::new(action_mod);
    flatten_agent.set_parent(&cg_data);
    flatten_agent.set_flatten_items(&cg_data);

    // action to test
    assert!(!flatten_agent.is_name_space_conflict(&cg_data));
}

#[test]
fn test_link_flatten_items_to_parent() {
    // preparation
    let mut cg_data = setup_processing_test(true)
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
        .unwrap()
        .check_impl_blocks()
        .unwrap()
        .process_external_dependencies()
        .unwrap()
        .fuse_challenge()
        .unwrap();

    let (fusion_node, _) = cg_data.get_fusion_bin_crate().unwrap();

    // test mod Action
    let action_mod = cg_data
        .iter_syn_items(fusion_node)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "action").then_some(n)
            } else {
                None
            }
        })
        .unwrap();

    let mut flatten_agent = FlattenAgent::new(action_mod);
    flatten_agent.set_parent(&cg_data);
    flatten_agent.set_flatten_items(&cg_data);

    // action to test
    flatten_agent.link_flatten_items_to_parent(&mut cg_data);

    let flattened_external_use_fmt_display = flatten_agent
        .flatten_items
        .iter()
        .find_map(|n| {
            if let Some(Item::Use(item_use)) = cg_data.get_syn_item(*n) {
                if let Some(name) = ItemName::from(item_use).get_ident_in_name_space() {
                    (name == "Display").then_some(*n)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        cg_data
            .get_syn_item(flattened_external_use_fmt_display)
            .unwrap()
            .to_token_stream()
            .to_string(),
        "use fmt :: Display ;"
    );
}

#[test]
fn test_check_use_statements() {
    // preparation
    let mut cg_data = setup_processing_test(true)
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
        .unwrap()
        .check_impl_blocks()
        .unwrap()
        .process_external_dependencies()
        .unwrap()
        .fuse_challenge()
        .unwrap();

    let (fusion_node, _) = cg_data.get_fusion_bin_crate().unwrap();

    // test mod MapPoint
    let map_point_mod = cg_data
        .iter_syn_items(fusion_node)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "my_map_point").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    // fusion of my_map_point does not contain further mod
    assert!(
        !cg_data
            .iter_syn_item_neighbors(map_point_mod)
            .any(|(_, i)| matches!(i, Item::Mod(_)))
    );
    let mut flatten_agent = FlattenAgent::new(map_point_mod);
    flatten_agent.set_parent(&cg_data);
    flatten_agent.set_flatten_items(&cg_data);
    flatten_agent.link_flatten_items_to_parent(&mut cg_data);
    flatten_agent.collect_sub_and_super_modules(&cg_data);

    // action to test
    flatten_agent.check_use_statements(&mut cg_data).unwrap();

    let action_mod = cg_data
        .iter_syn_items(fusion_node)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "action").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    let action_use_of_map_point = cg_data
        .iter_syn_item_neighbors(action_mod)
        .find_map(|(n, i)| {
            if let Some(name) = ItemName::from(i).get_ident_in_name_space() {
                (name == "MapPoint").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    let Some(item) = cg_data.get_syn_item(action_use_of_map_point) else {
        panic!("Expected use item.");
    };
    assert_eq!(
        item.to_token_stream().to_string(),
        "use super :: super :: my_map_two_dim :: MapPoint ;"
    );
}

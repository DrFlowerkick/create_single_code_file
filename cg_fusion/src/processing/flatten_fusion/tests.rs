// contains all tests of flatten_fusion

use quote::ToTokens;
use syn::Item;

use crate::parsing::ItemName;
use crate::processing::flatten_fusion::FlattenAgent;

use super::super::tests::setup_processing_test;
use super::*;

#[test]
fn test_transform_use_and_path_statements_starting_with_crate_keyword_to_relative() {
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

    let (fusion_crate, _) = cg_data.get_fusion_bin_crate().unwrap();

    // action to test
    cg_data
        .transform_use_and_path_statements_starting_with_crate_keyword_to_relative(fusion_crate)
        .unwrap();

    let main_use_statements: Vec<String> = cg_data
        .iter_syn_item_neighbors(fusion_crate)
        .filter_map(|(_, i)| {
            if let Item::Use(item_use) = i {
                Some(item_use.to_token_stream().to_string())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        main_use_statements,
        [
            "use cg_fusion_binary_test :: action :: Action ;",
            "use my_map_two_dim :: my_map_point :: MapPoint ;",
            "use cg_fusion_binary_test :: Go ;",
            "use cg_fusion_binary_test :: X ;",
            "use cg_fusion_binary_test :: Y ;",
        ]
    );

    // test mod action
    let action_mod = cg_data
        .iter_syn_items(fusion_crate)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "action").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    let action_use_map_point: String = cg_data
        .iter_syn_item_neighbors(action_mod)
        .find_map(|(_, i)| {
            if let Item::Use(item_use) = i {
                if let Some(name) = ItemName::from(item_use).get_ident_in_name_space() {
                    (name == "MapPoint").then_some(item_use.to_token_stream().to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        action_use_map_point,
        "use super :: super :: my_map_two_dim :: my_map_point :: MapPoint ;"
    );

    // test mod cg_fusion_binary_test
    let cg_fusion_binary_test_mod = cg_data
        .iter_syn_items(fusion_crate)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "cg_fusion_binary_test").then_some(n)
            } else {
                None
            }
        })
        .unwrap();
    let cg_fusion_binary_test_mod_use_my_map2dim: String = cg_data
        .iter_syn_item_neighbors(cg_fusion_binary_test_mod)
        .find_map(|(_, i)| {
            if let Item::Use(item_use) = i {
                if let Some(name) = ItemName::from(item_use).get_ident_in_name_space() {
                    (name == "MyMap2D").then_some(item_use.to_token_stream().to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        cg_fusion_binary_test_mod_use_my_map2dim,
        "use super :: my_map_two_dim :: MyMap2D ;"
    );
}

#[test]
fn test_set_parent() {
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

    let (fusion_crate, _) = cg_data.get_fusion_bin_crate().unwrap();

    cg_data
        .transform_use_and_path_statements_starting_with_crate_keyword_to_relative(fusion_crate)
        .unwrap();

    // test mod MapPoint
    let map_point_mod = cg_data
        .iter_syn_items(fusion_crate)
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
    flatten_agent.set_parent(&cg_data).unwrap();

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
        .iter_syn_items(fusion_crate)
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
    flatten_agent.set_parent(&cg_data).unwrap();

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

    let (fusion_crate, _) = cg_data.get_fusion_bin_crate().unwrap();

    cg_data
        .transform_use_and_path_statements_starting_with_crate_keyword_to_relative(fusion_crate)
        .unwrap();

    // test mod MapPoint
    let map_point_mod = cg_data
        .iter_syn_items(fusion_crate)
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
    flatten_agent.set_parent(&cg_data).unwrap();

    // action to test
    flatten_agent.set_flatten_items(&cg_data).unwrap();

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
        .iter_syn_items(fusion_crate)
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
    flatten_agent.set_parent(&cg_data).unwrap();

    // action to test
    flatten_agent.set_flatten_items(&cg_data).unwrap();

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

    let (fusion_crate, _) = cg_data.get_fusion_bin_crate().unwrap();

    cg_data
        .transform_use_and_path_statements_starting_with_crate_keyword_to_relative(fusion_crate)
        .unwrap();

    // test mod MapPoint
    let map_point_mod = cg_data
        .iter_syn_items(fusion_crate)
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
    flatten_agent.set_parent(&cg_data).unwrap();
    flatten_agent.set_flatten_items(&cg_data).unwrap();

    // action to test
    assert!(!flatten_agent.is_name_space_conflict(&cg_data));

    // test mod Action
    let action_mod = cg_data
        .iter_syn_items(fusion_crate)
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
    flatten_agent.set_parent(&cg_data).unwrap();
    flatten_agent.set_flatten_items(&cg_data).unwrap();

    // action to test
    assert!(!flatten_agent.is_name_space_conflict(&cg_data));
}

#[test]
fn test_set_sub_and_super_nodes() {
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

    let (fusion_crate, _) = cg_data.get_fusion_bin_crate().unwrap();

    cg_data
        .transform_use_and_path_statements_starting_with_crate_keyword_to_relative(fusion_crate)
        .unwrap();

    // test mod Action
    let action_mod = cg_data
        .iter_syn_items(fusion_crate)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "action").then_some(n)
            } else {
                None
            }
        })
        .unwrap();

    let mut flatten_agent = FlattenAgent::new(action_mod);
    flatten_agent.set_parent(&cg_data).unwrap();
    flatten_agent.set_flatten_items(&cg_data).unwrap();

    // action to test
    flatten_agent.set_sub_and_super_nodes(fusion_crate, &cg_data);

    let fusion_crate_items_in_super_nodes: Vec<String> = cg_data
        .iter_syn_item_neighbors(fusion_crate)
        .filter_map(|(n, _)| {
            if flatten_agent.super_check_items.contains(&n) {
                cg_data.get_verbose_name_of_tree_node(n).ok()
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        fusion_crate_items_in_super_nodes,
        [
            "Action (Use)",
            "main (Fn)",
            "MapPoint (Use)",
            "Go (Use)",
            "X (Use)",
            "Y (Use)"
        ]
    );
}

#[test]
fn test_pre_linking_use_and_path_fixing() {
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

    let (fusion_crate, _) = cg_data.get_fusion_bin_crate().unwrap();

    cg_data
        .transform_use_and_path_statements_starting_with_crate_keyword_to_relative(fusion_crate)
        .unwrap();

    // test mod Action
    let action_mod = cg_data
        .iter_syn_items(fusion_crate)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "action").then_some(n)
            } else {
                None
            }
        })
        .unwrap();

    let mut flatten_agent = FlattenAgent::new(action_mod);
    flatten_agent.set_parent(&cg_data).unwrap();
    flatten_agent.set_flatten_items(&cg_data).unwrap();
    flatten_agent.set_sub_and_super_nodes(fusion_crate, &cg_data);

    // action to test
    flatten_agent.pre_linking_use_and_path_fixing(&mut cg_data).unwrap();

    let action_use_display = cg_data
        .iter_syn_item_neighbors(action_mod)
        .find_map(|(_, i)| {
            if let Item::Use(item_use) = i {
                if let Some(name) = ItemName::from(item_use).get_ident_in_name_space() {
                    (name == "Display").then_some(item_use.to_token_stream().to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(action_use_display, "use fmt :: Display ;");

    let action_use_map_point = cg_data
        .iter_syn_item_neighbors(action_mod)
        .find_map(|(_, i)| {
            if let Item::Use(item_use) = i {
                if let Some(name) = ItemName::from(item_use).get_ident_in_name_space() {
                    (name == "MapPoint").then_some(item_use.to_token_stream().to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        action_use_map_point,
        "use super :: my_map_two_dim :: my_map_point :: MapPoint ;"
    );
}

#[test]
fn test_post_linking_use_and_path_fixing() {
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

    let (fusion_crate, _) = cg_data.get_fusion_bin_crate().unwrap();

    cg_data
        .transform_use_and_path_statements_starting_with_crate_keyword_to_relative(fusion_crate)
        .unwrap();

    // test mod MapPoint
    let map_point_mod = cg_data
        .iter_syn_items(fusion_crate)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "my_map_point").then_some(n)
            } else {
                None
            }
        })
        .unwrap();

    let mut flatten_agent = FlattenAgent::new(map_point_mod);
    flatten_agent.set_parent(&cg_data).unwrap();
    flatten_agent.set_flatten_items(&cg_data).unwrap();
    flatten_agent.set_sub_and_super_nodes(fusion_crate, &cg_data);
    flatten_agent.pre_linking_use_and_path_fixing(&mut cg_data).unwrap();
    flatten_agent.link_flatten_items_to_parent(&mut cg_data);

    // action to test
    flatten_agent
        .post_linking_use_and_path_fixing(&mut cg_data)
        .unwrap();

    // check action mode use of MapPoint
    let action_mod = cg_data
        .iter_syn_items(fusion_crate)
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
            if let Item::Use(item_use) = i {
                if let Some(name) = ItemName::from(item_use).get_ident_in_name_space() {
                    (name == "MapPoint").then_some(n)
                } else {
                    None
                }
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

    // check fusion crate use of MapPoint
    let fusion_crate_use_of_map_point = cg_data
        .iter_syn_item_neighbors(fusion_crate)
        .find_map(|(n, i)| {
            if let Item::Use(item_use) = i {
                if let Some(name) = ItemName::from(item_use).get_ident_in_name_space() {
                    (name == "MapPoint").then_some(n)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    let Some(item) = cg_data.get_syn_item(fusion_crate_use_of_map_point) else {
        panic!("Expected use item.");
    };
    assert_eq!(
        item.to_token_stream().to_string(),
        "use my_map_two_dim :: MapPoint ;"
    );
}

#[test]
fn test_set_order_of_flattened_items_in_parent() {
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

    let (fusion_crate, _) = cg_data.get_fusion_bin_crate().unwrap();

    cg_data
        .transform_use_and_path_statements_starting_with_crate_keyword_to_relative(fusion_crate)
        .unwrap();

    // test mod MapPoint
    let map_point_mod = cg_data
        .iter_syn_items(fusion_crate)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "my_map_point").then_some(n)
            } else {
                None
            }
        })
        .unwrap();

    let mut flatten_agent = FlattenAgent::new(map_point_mod);
    flatten_agent.set_parent(&cg_data).unwrap();
    flatten_agent.set_flatten_items(&cg_data).unwrap();
    flatten_agent.set_sub_and_super_nodes(fusion_crate, &cg_data);
    flatten_agent.pre_linking_use_and_path_fixing(&mut cg_data).unwrap();
    flatten_agent.link_flatten_items_to_parent(&mut cg_data);
    flatten_agent
        .post_linking_use_and_path_fixing(&mut cg_data)
        .unwrap();

    // action to test
    flatten_agent
        .set_order_of_flattened_items_in_parent(&mut cg_data)
        .unwrap();

    let parent_items: Vec<String> = cg_data
        .get_sorted_mod_content(flatten_agent.parent)
        .unwrap()
        .iter()
        .map(|i| format!("{}", ItemName::from(i)))
        .collect();
    assert_eq!(
        parent_items,
        [
            "MapPoint (Struct)",
            "impl<constX:usize,constY:usize> MapPoint<X,Y>",
            "MyMap2D (Struct)",
            "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>",
            "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>"
        ]
    );
    // test mod action
    let action_mod = cg_data
        .iter_syn_items(fusion_crate)
        .find_map(|(n, i)| {
            if let Item::Mod(item_mod) = i {
                (item_mod.ident == "action").then_some(n)
            } else {
                None
            }
        })
        .unwrap();

    let mut flatten_agent = FlattenAgent::new(action_mod);
    flatten_agent.set_parent(&cg_data).unwrap();
    flatten_agent.set_flatten_items(&cg_data).unwrap();
    flatten_agent.set_sub_and_super_nodes(fusion_crate, &cg_data);
    flatten_agent.pre_linking_use_and_path_fixing(&mut cg_data).unwrap();
    flatten_agent.link_flatten_items_to_parent(&mut cg_data);
    flatten_agent
        .post_linking_use_and_path_fixing(&mut cg_data)
        .unwrap();

    // action to test
    flatten_agent
        .set_order_of_flattened_items_in_parent(&mut cg_data)
        .unwrap();

    let parent_items: Vec<String> = cg_data
        .get_sorted_mod_content(flatten_agent.parent)
        .unwrap()
        .iter()
        .map(|i| format!("{}", ItemName::from(i)))
        .collect();
    assert_eq!(
        parent_items,
        [
            "Display (Use)",
            "MapPoint (Use)",
            "Action (Struct)",
            "impl Display for Action",
            "impl Action",
            "MyMap2D (Use)",
            "fmt (Use)",
            "X (Const)",
            "Y (Const)",
            "N (Const)",
            "Value (Enum)",
            "impl fmt::Display for Value",
            "Go (Struct)",
            "impl Default for Go",
            "impl Go"
        ]
    );
}

#[test]
fn flatten_fusion() {
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
        .unwrap()
        .flatten_fusion()
        .unwrap();

    let (fusion_crate, _) = cg_data.get_fusion_bin_crate().unwrap();

    // no mod in fusion crate
    assert!(
        !cg_data
            .iter_syn_items(fusion_crate)
            .any(|(_, i)| matches!(i, Item::Mod(_)))
    );
}

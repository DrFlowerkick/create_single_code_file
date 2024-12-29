// test cases for usage.rs

use syn::Ident;

use crate::parsing::ItemName;
use crate::parsing::PathAnalysis;

use super::super::tests::setup_analyze_test;
use super::*;

#[test]
fn test_get_path_target() {
    // preparation
    let mut cg_data = setup_analyze_test();
    cg_data.add_challenge_dependencies().unwrap();
    cg_data.add_bin_src_files_of_challenge().unwrap();
    cg_data.add_lib_src_files().unwrap();

    // get use entries from cg_fusion_lib_test
    let (cg_fusion_lib_test_index, _) = cg_data
        .iter_lib_crates()
        .find(|(_, c)| c.name == "cg_fusion_lib_test")
        .unwrap();

    // expand use group in cg_fusion_lib_test for testing (see below MyMap2D and MapPoint)
    let (use_group_index, _) = cg_data
        .iter_syn_neighbors(cg_fusion_lib_test_index)
        .find(|(_, i)| i.contains_use_group())
        .unwrap();
    cg_data.expand_use_group(use_group_index).unwrap();

    let use_statements_of_cg_fusion_lib_test: Vec<(NodeIndex, Ident, &UseTree)> = cg_data
        .iter_syn_neighbors(cg_fusion_lib_test_index)
        .filter_map(|(n, i)| {
            if let Item::Use(item_use) = i {
                ItemName::from(i)
                    .extract_ident()
                    .map(|id| (n, id, &item_use.tree))
            } else {
                None
            }
        })
        .collect();
    // get use entries, which point to lib crates
    let (use_index_my_map_two_dim, _, use_tree_my_map_two_dim) =
        use_statements_of_cg_fusion_lib_test
            .iter()
            .find(|(_, id, _)| id == "my_map_two_dim")
            .unwrap();
    let (use_index_my_array, _, use_tree_my_array) = use_statements_of_cg_fusion_lib_test
        .iter()
        .find(|(_, id, _)| id == "my_array")
        .unwrap();
    let (my_map_two_dim_mod_index, _) = cg_data
        .iter_lib_crates()
        .find(|(_, c)| c.name == "my_map_two_dim")
        .unwrap();
    let (my_array_mod_index, _) = cg_data
        .iter_lib_crates()
        .find(|(_, c)| c.name == "my_array")
        .unwrap();

    // test path target of use items, which point to lib crates
    assert_eq!(
        cg_data
            .get_path_target(*use_index_my_map_two_dim, *use_tree_my_map_two_dim)
            .unwrap(),
        PathTarget::Item(my_map_two_dim_mod_index)
    );
    assert_eq!(
        cg_data
            .get_path_target(*use_index_my_array, *use_tree_my_array)
            .unwrap(),
        PathTarget::Item(my_array_mod_index)
    );

    // get use entries, which point to items in modules
    let (use_index_my_array_struct, _, use_tree_my_array_struct) =
        use_statements_of_cg_fusion_lib_test
            .iter()
            .find(|(_, id, _)| id == "MyArray")
            .unwrap();
    let (use_index_my_map_2d_struct, _, use_tree_my_map_2d_struct) =
        use_statements_of_cg_fusion_lib_test
            .iter()
            .find(|(_, id, _)| id == "MyMap2D")
            .unwrap();
    let (use_index_my_map_point_struct, _, use_tree_my_map_point_struct) =
        use_statements_of_cg_fusion_lib_test
            .iter()
            .find(|(_, id, _)| id == "MapPoint")
            .unwrap();
    // get item indices
    let (my_map_point_mod_index, _) = cg_data
        .iter_syn_items(my_map_two_dim_mod_index)
        .filter_map(|(n, i)| {
            if let Item::Mod(_) = i {
                ItemName::from(i).extract_ident().map(|id| (n, id))
            } else {
                None
            }
        })
        .find(|(_, c)| c == "my_map_point")
        .unwrap();
    let my_array_item_index = cg_data
        .iter_syn_neighbors(my_array_mod_index)
        .filter_map(|(n, i)| ItemName::from(i).extract_ident().map(|id| (n, id)))
        .find(|(_, id)| id == "MyArray")
        .unwrap()
        .0;
    let my_map_2d_item_index = cg_data
        .iter_syn_neighbors(my_map_two_dim_mod_index)
        .filter_map(|(n, i)| ItemName::from(i).extract_ident().map(|id| (n, id)))
        .find(|(_, id)| id == "MyMap2D")
        .unwrap()
        .0;
    let my_map_point_item_index = cg_data
        .iter_syn_neighbors(my_map_point_mod_index)
        .filter_map(|(n, i)| ItemName::from(i).extract_ident().map(|id| (n, id)))
        .find(|(_, id)| id == "MapPoint")
        .unwrap()
        .0;
    // test path target of use items, which point to items in modules
    assert_eq!(
        cg_data
            .get_path_target(*use_index_my_array_struct, *use_tree_my_array_struct)
            .unwrap(),
        PathTarget::Item(my_array_item_index)
    );
    assert_eq!(
        cg_data
            .get_path_target(*use_index_my_map_2d_struct, *use_tree_my_map_2d_struct)
            .unwrap(),
        PathTarget::Item(my_map_2d_item_index)
    );
    assert_eq!(
        cg_data
            .get_path_target(
                *use_index_my_map_point_struct,
                *use_tree_my_map_point_struct
            )
            .unwrap(),
        PathTarget::Item(my_map_point_item_index)
    );

    // get use entries, which point to use globs
    let use_globs: Vec<(NodeIndex, Ident, &UseTree)> = cg_data
        .iter_syn_neighbors(my_map_two_dim_mod_index)
        .filter_map(|(n, i)| {
            if let Item::Use(item_use) = i {
                let path = item_use.tree.extract_path().unwrap();
                ItemName::from(i).is_glob().then_some((
                    n,
                    path.segments.last().unwrap().to_owned(),
                    &item_use.tree,
                ))
            } else {
                None
            }
        })
        .collect();

    let (use_glob_index_my_map_point, _, use_glob_tree_my_map_point) = use_globs
        .iter()
        .find(|(_, id, _)| id == "my_map_point")
        .unwrap();
    let (use_glob_index_my_array, _, use_glob_tree_my_array) = use_globs
        .iter()
        .find(|(_, id, _)| id == "my_array")
        .unwrap();
    let (use_glob_index_my_compass, _, use_glob_tree_my_compass) = use_globs
        .iter()
        .find(|(_, id, _)| id == "my_compass")
        .unwrap();

    let my_compass_mod_index = cg_data
        .iter_syn_neighbors(my_map_point_mod_index)
        .filter_map(|(n, i)| {
            if let Item::Mod(_) = i {
                ItemName::from(i).extract_ident().map(|id| (n, id))
            } else {
                None
            }
        })
        .find(|(_, id)| id == "my_compass")
        .unwrap()
        .0;

    // test path target of use items, which point to use globs
    assert_eq!(
        cg_data
            .get_path_target(*use_glob_index_my_map_point, *use_glob_tree_my_map_point)
            .unwrap(),
        PathTarget::Glob(my_map_point_mod_index)
    );
    assert_eq!(
        cg_data
            .get_path_target(*use_glob_index_my_array, *use_glob_tree_my_array)
            .unwrap(),
        PathTarget::Glob(my_array_mod_index)
    );
    assert_eq!(
        cg_data
            .get_path_target(*use_glob_index_my_compass, *use_glob_tree_my_compass)
            .unwrap(),
        PathTarget::Glob(my_compass_mod_index)
    );

    // get use entries of my_map_point
    let use_of_my_map_point: Vec<(NodeIndex, Ident, &UseTree)> = cg_data
        .iter_syn_neighbors(my_map_point_mod_index)
        .filter_map(|(n, i)| {
            if let Item::Use(item_use) = i {
                let path = item_use.tree.extract_path().unwrap();
                Some((n, path.segments.last().unwrap().to_owned(), &item_use.tree))
            } else {
                None
            }
        })
        .collect();
    let (use_extern_ordering, _, use_tree_extern_ordering) = use_of_my_map_point
        .iter()
        .find(|(_, id, _)| id == "Ordering")
        .unwrap();
    let (use_glob_index_my_compass, _, use_glob_tree_my_compass) = use_of_my_map_point
        .iter()
        .find(|(_, id, _)| id == "my_compass")
        .unwrap();
    assert_eq!(
        cg_data
            .get_path_target(*use_extern_ordering, *use_tree_extern_ordering)
            .unwrap(),
        PathTarget::ExternalPackage
    );
    assert_eq!(
        cg_data
            .get_path_target(*use_glob_index_my_compass, *use_glob_tree_my_compass)
            .unwrap(),
        PathTarget::Glob(my_compass_mod_index)
    );
}

#[test]
fn test_is_visible_for_module() {
    // preparation
    let mut cg_data = setup_analyze_test();
    cg_data.add_challenge_dependencies().unwrap();
    cg_data.add_bin_src_files_of_challenge().unwrap();
    cg_data.add_lib_src_files().unwrap();

    // get module index of my_compass and my_map_point
    let (my_map_two_dim_mod_index, _) = cg_data
        .iter_lib_crates()
        .find(|(_, c)| c.name == "my_map_two_dim")
        .unwrap();
    let my_compass_mod_index = cg_data
        .iter_syn_items(my_map_two_dim_mod_index)
        .filter_map(|(n, i)| {
            if let Item::Mod(_) = i {
                ItemName::from(i).extract_ident().map(|id| (n, id))
            } else {
                None
            }
        })
        .find(|(_, id)| id == "my_compass")
        .unwrap()
        .0;
    let my_map_point_mod_index = cg_data
        .iter_syn_items(my_map_two_dim_mod_index)
        .filter_map(|(n, i)| {
            if let Item::Mod(_) = i {
                ItemName::from(i).extract_ident().map(|id| (n, id))
            } else {
                None
            }
        })
        .find(|(_, id)| id == "my_map_point")
        .unwrap()
        .0;
    // get visible items in my_map_point for my_compass
    let visible_items_with_ident_of_my_map_point_for_my_compass: Vec<Ident> = cg_data
        .iter_syn_neighbors(my_map_point_mod_index)
        .filter_map(|(n, i)| {
            ItemName::from(i)
                .extract_ident()
                .map(|id| {
                    cg_data
                        .is_visible_for_module(n, my_compass_mod_index)
                        .ok()
                        .map(|vis| vis.then_some(id))
                        .flatten()
                })
                .flatten()
        })
        .collect();
    // test visibility of items in my_map_point for my_compass
    assert_eq!(
        visible_items_with_ident_of_my_map_point_for_my_compass,
        vec![
            "OrientationIter",
            "NeighborIter",
            "MapPoint",
            "Ordering",
            "my_compass"
        ]
    );
    // test single use glob is visible
    assert_eq!(
        cg_data
            .iter_syn_neighbors(my_map_point_mod_index)
            .filter_map(|(n, i)| {
                ItemName::from(i)
                    .is_glob()
                    .then_some(cg_data.is_visible_for_module(n, my_compass_mod_index).ok())
                    .flatten()
            })
            .count(),
        1
    );
    // get visible items in my_map_two_dim for my_compass
    let visible_items_with_ident_of_my_map_two_dim_for_my_compass: Vec<Ident> = cg_data
        .iter_syn_neighbors(my_map_two_dim_mod_index)
        .filter_map(|(n, i)| {
            ItemName::from(i)
                .extract_ident()
                .map(|id| {
                    cg_data
                        .is_visible_for_module(n, my_compass_mod_index)
                        .ok()
                        .map(|vis| vis.then_some(id))
                        .flatten()
                })
                .flatten()
        })
        .collect();
    // test visibility of items in my_map_two_dim for my_compass
    assert_eq!(
        visible_items_with_ident_of_my_map_two_dim_for_my_compass,
        vec![
            "DistanceIter",
            "FilterFn",
            "MyMap2D",
            "IsCellFreeFn",
            "my_map_point"
        ]
    );
    // test use globs are visible
    assert_eq!(
        cg_data
            .iter_syn_neighbors(my_map_two_dim_mod_index)
            .filter_map(|(n, i)| {
                ItemName::from(i)
                    .is_glob()
                    .then_some(cg_data.is_visible_for_module(n, my_compass_mod_index).ok())
                    .flatten()
            })
            .count(),
        3
    );
    // get visible items in my_map_point for my_map_two_dim
    let visible_items_with_ident_of_my_map_point_for_my_map_two_dim: Vec<Ident> = cg_data
        .iter_syn_neighbors(my_map_point_mod_index)
        .filter_map(|(n, i)| {
            ItemName::from(i)
                .extract_ident()
                .map(|id| {
                    cg_data
                        .is_visible_for_module(n, my_map_two_dim_mod_index)
                        .ok()
                        .map(|vis| vis.then_some(id))
                        .flatten()
                })
                .flatten()
        })
        .collect();
    // test visibility of items in my_map_point for my_map_two_dim
    assert_eq!(
        visible_items_with_ident_of_my_map_point_for_my_map_two_dim,
        vec!["MapPoint", "my_compass"]
    );
    // get visible items in my_compass for my_map_two_dim
    let visible_items_with_ident_of_my_compass_for_my_map_two_dim: Vec<Ident> = cg_data
        .iter_syn_neighbors(my_compass_mod_index)
        .filter_map(|(n, i)| {
            ItemName::from(i)
                .extract_ident()
                .map(|id| {
                    cg_data
                        .is_visible_for_module(n, my_map_two_dim_mod_index)
                        .ok()
                        .map(|vis| vis.then_some(id))
                        .flatten()
                })
                .flatten()
        })
        .collect();
    // test visibility of items in my_map_point for my_map_two_dim
    assert_eq!(
        visible_items_with_ident_of_my_compass_for_my_map_two_dim,
        vec!["Compass"]
    );
}


#[test]
fn test_expand_use_groups() {
    // preparation
    let mut cg_data = setup_analyze_test();
    cg_data.add_challenge_dependencies().unwrap();
    cg_data.add_bin_src_files_of_challenge().unwrap();
    cg_data.add_lib_src_files().unwrap();

    // number of use statements before expansion in challenge bin crate
    let (challenge_bin_crate_index, _) = cg_data.get_challenge_bin_crate().unwrap();
    assert_eq!(
        cg_data
            .iter_syn_neighbors(challenge_bin_crate_index)
            .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
            .count(),
        3
    );
    // number of use statements before expansion in cg_fusion_lib_test lib crate
    let (cg_fusion_lib_test_index, _) = cg_data
        .iter_lib_crates()
        .find(|(_, c)| c.name == "cg_fusion_lib_test")
        .unwrap();
    assert_eq!(
        cg_data
            .iter_syn_neighbors(cg_fusion_lib_test_index)
            .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
            .count(),
        5
    );

    // action to test
    cg_data.expand_use_groups().unwrap();

    // number of use statements after expansion in challenge bin crate
    let (challenge_bin_crate_index, _) = cg_data.get_challenge_bin_crate().unwrap();
    assert_eq!(
        cg_data
            .iter_syn_neighbors(challenge_bin_crate_index)
            .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
            .count(),
        5
    );
    let use_statements: Vec<String> = cg_data
        .iter_syn_neighbors(challenge_bin_crate_index)
        .filter_map(|(_, i)| match i {
            Item::Use(use_item) => Some(use_item.to_token_stream().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        use_statements,
        vec![
            "use cg_fusion_binary_test :: Y ;",
            "use cg_fusion_binary_test :: X ;",
            "use cg_fusion_binary_test :: Go ;",
            "use cg_fusion_lib_test :: my_map_two_dim :: my_map_point :: * ;",
            "use cg_fusion_binary_test :: action :: Action ;",
        ]
    );
    // number of use statements after expansion in cg_fusion_lib_test lib crate
    let (cg_fusion_lib_test_index, _) = cg_data
        .iter_lib_crates()
        .find(|(_, c)| c.name == "cg_fusion_lib_test")
        .unwrap();
    assert_eq!(
        cg_data
            .iter_syn_neighbors(cg_fusion_lib_test_index)
            .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
            .count(),
        6
    );
}

#[test]
fn test_expand_use_globs_and_link_use_items() {
    // preparation
    let mut cg_data = setup_analyze_test();
    cg_data.add_challenge_dependencies().unwrap();
    cg_data.add_bin_src_files_of_challenge().unwrap();
    cg_data.add_lib_src_files().unwrap();
    cg_data.expand_use_groups().unwrap();

    // action to test
    cg_data.expand_use_globs_and_link_use_items().unwrap();

    // assert use statements after expansion of globs in challenge bin crate
    let (challenge_bin_crate_index, _) = cg_data.get_challenge_bin_crate().unwrap();
    let use_statements: Vec<String> = cg_data
        .iter_syn_neighbors(challenge_bin_crate_index)
        .filter_map(|(_, i)| match i {
            Item::Use(use_item) => Some(use_item.to_token_stream().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        use_statements,
        vec![
            "use cg_fusion_lib_test :: my_map_two_dim :: my_map_point :: my_compass ;",
            "use cg_fusion_lib_test :: my_map_two_dim :: my_map_point :: MapPoint ;",
            "use cg_fusion_binary_test :: Y ;",
            "use cg_fusion_binary_test :: X ;",
            "use cg_fusion_binary_test :: Go ;",
            "use cg_fusion_binary_test :: action :: Action ;",
        ]
    );

    let (my_map_two_dim_mod_index, _) = cg_data
        .iter_lib_crates()
        .find(|(_, c)| c.name == "my_map_two_dim")
        .unwrap();
    // using iter_syn_items() here to also collect use statements of my_map_point, which is a module of my_map_two_dim
    let use_statements: Vec<String> = cg_data
        .iter_syn_items(my_map_two_dim_mod_index)
        .filter_map(|(_, i)| match i {
            Item::Use(use_item) => Some(use_item.to_token_stream().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        use_statements,
        vec![
            "use my_map_point :: my_compass :: Compass ;",
            "use my_array :: MyArray ;",
            "use self :: my_map_point :: my_compass ;",
            "use self :: my_map_point :: MapPoint ;",
            "use crate :: my_map_point :: my_compass :: Compass ;",
            "use std :: cmp :: Ordering ;",
        ]
    );

    let (cg_fusion_binary_test_index, _) = cg_data
        .iter_lib_crates()
        .find(|(_, c)| c.name == "cg_fusion_binary_test")
        .unwrap();
    // using iter_syn_items() here to also collect use statements of action, which is a module of cg_fusion_binary_test
    let use_statements: Vec<String> = cg_data
        .iter_syn_items(cg_fusion_binary_test_index)
        .filter_map(|(_, i)| match i {
            Item::Use(use_item) => Some(use_item.to_token_stream().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        use_statements,
        vec![
            "use cg_fusion_lib_test :: my_map_two_dim :: my_map_point ;",
            "use cg_fusion_lib_test :: my_map_two_dim :: IsCellFreeFn ;",
            "use cg_fusion_lib_test :: my_map_two_dim :: MyMap2D ;",
            "use cg_fusion_lib_test :: my_map_two_dim :: FilterFn ;",
            "use crate :: action :: Action ;",
            "use std :: fmt ;",
            "use super :: action ;",
            "use super :: X ;",
            "use super :: Y ;",
            "use super :: Value ;",
            "use super :: Go ;",
            "use cg_fusion_lib_test :: my_map_two_dim :: my_map_point :: MapPoint ;",
        ]
    );
}

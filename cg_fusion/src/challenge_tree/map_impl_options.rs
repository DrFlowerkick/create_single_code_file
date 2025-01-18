// fn to map impl options from config to node indices of impl items

use super::{ChallengeTreeError, TreeResult};

use crate::parsing::ItemName;
use crate::{add_context, configuration::CgCli, CgData};
use anyhow::anyhow;
use petgraph::stable_graph::NodeIndex;
use serde::Deserialize;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fs;
use syn::Item;

#[derive(Debug, Deserialize, Default)]
struct ImplOptions {
    include_impl_items: Vec<String>,
    exclude_impl_items: Vec<String>,
}

#[derive(Debug)]
enum NameParsingState {
    CheckForCrate,
    CheckForModule,
    NextModule,
    UserDefinedType,
    ImplItem,
}

impl NameParsingState {
    fn next_module_or_user_defined_type(
        &mut self,
        num_path_elements: usize,
        index_path_element: usize,
    ) {
        assert!(num_path_elements > index_path_element);
        *self = match num_path_elements - index_path_element {
            3.. => NameParsingState::NextModule,
            2 => NameParsingState::UserDefinedType,
            ..=1 => panic!(
                "{}",
                add_context!("Expected num_path_elements to be >= index_path_element + 2")
            ),
        };
    }
}

#[derive(Debug)]
enum ProcessOption {
    Include,
    Exclude,
}

impl<O: CgCli, S> CgData<O, S> {
    pub(crate) fn map_impl_config_options_to_node_indices(
        &self,
    ) -> TreeResult<HashMap<NodeIndex, bool>> {
        let mut impl_options: HashMap<NodeIndex, bool> = HashMap::new();
        // load config file if existing
        let impl_config = if let Some(ref toml_config_path) = self.options.input().impl_item_toml {
            let toml_string = fs::read_to_string(toml_config_path)?;
            let toml_options: ImplOptions = toml::from_str(&toml_string)?;
            toml_options
        } else {
            ImplOptions::default()
        };
        // Collect all impl items to include or exclude.
        // If index is already in include, include wins.
        for (impl_item, process_option) in self
            .options
            .input()
            .include_impl_item
            .iter()
            .chain(impl_config.include_impl_items.iter())
            .map(|ii| (ii, ProcessOption::Include))
            .chain(
                self.options
                    .input()
                    .exclude_impl_item
                    .iter()
                    .chain(impl_config.exclude_impl_items.iter())
                    .map(|ii| (ii, ProcessOption::Exclude)),
            )
        {
            self.collect_impl_config_option_indices(impl_item, process_option, &mut impl_options)?;
        }
        Ok(impl_options)
    }

    fn collect_impl_config_option_indices(
        &self,
        impl_item: &str,
        process_option: ProcessOption,
        impl_options: &mut HashMap<NodeIndex, bool>,
    ) -> TreeResult<()> {
        let impl_item_path_elements: Vec<&str> = impl_item.split("::").collect();
        let mut name_parsing_mode = match impl_item_path_elements.len() {
            0 => Err(anyhow!(add_context!("Expected name of impl item.")))?,
            1 => NameParsingState::ImplItem,
            2 => NameParsingState::UserDefinedType,
            3.. => NameParsingState::CheckForCrate,
        };
        let mut current_node_index: Option<NodeIndex> = None;
        let mut index_path_element = 0;
        loop {
            if let Some(&path_element) = impl_item_path_elements.get(index_path_element) {
                match name_parsing_mode {
                    NameParsingState::CheckForCrate => {
                        if let Some((crate_index, _, _)) = self
                            .iter_crates()
                            .find(|(_, _, cf)| cf.name == *path_element)
                        {
                            current_node_index = Some(crate_index);
                            index_path_element += 1;
                            name_parsing_mode.next_module_or_user_defined_type(
                                impl_item_path_elements.len(),
                                index_path_element,
                            );
                        } else {
                            name_parsing_mode = NameParsingState::CheckForModule;
                        }
                    }
                    NameParsingState::CheckForModule => {
                        let modules: Vec<NodeIndex> = self
                            .iter_crates()
                            .flat_map(|(n, _, _)| self.iter_syn_items(n))
                            .filter_map(|(n, i)| match i {
                                Item::Mod(_) => ItemName::from(i)
                                    .get_ident_in_name_space()
                                    .map(|id| (n, id)),
                                _ => None,
                            })
                            .filter_map(|(n, id)| (id == path_element).then_some(n))
                            .collect();
                        match modules.len() {
                            0 => {
                                return Err(ChallengeTreeError::NotExistingImplItemOfConfig(
                                    impl_item.to_owned(),
                                ))
                            }
                            1 => {
                                current_node_index = Some(modules[0]);
                                index_path_element += 1;
                                name_parsing_mode.next_module_or_user_defined_type(
                                    impl_item_path_elements.len(),
                                    index_path_element,
                                );
                            }
                            2.. => {
                                return Err(ChallengeTreeError::NotUniqueImplItemOfConfig(
                                    impl_item.to_owned(),
                                ))
                            }
                        }
                    }
                    NameParsingState::NextModule => {
                        if let Some(module_index) = current_node_index {
                            if let Some((next_module_index, _)) = self
                                .iter_syn_item_neighbors(module_index)
                                .filter_map(|(n, i)| match i {
                                    Item::Mod(_) => ItemName::from(i)
                                        .get_ident_in_name_space()
                                        .map(|id| (n, id)),
                                    _ => None,
                                })
                                .find(|(_, id)| id == path_element)
                            {
                                current_node_index = Some(next_module_index);
                                index_path_element += 1;
                                name_parsing_mode.next_module_or_user_defined_type(
                                    impl_item_path_elements.len(),
                                    index_path_element,
                                );
                            }
                        } else {
                            unreachable!("Expected module index.");
                        }
                    }
                    NameParsingState::UserDefinedType => {
                        if let Some(module_index) = current_node_index {
                            // search in all impl blocks of current module for impl items with given impl name
                            let impl_item_indices: Vec<NodeIndex> = self
                                .iter_syn_item_neighbors(module_index)
                                .filter(|(_, i)| match i {
                                    Item::Impl(item_impl) => {
                                        if let ItemName::TypeStringAndNameString(_, name) =
                                            ItemName::from(*i)
                                        {
                                            item_impl.trait_.is_none() && name == *path_element
                                        } else {
                                            false
                                        }
                                    }
                                    _ => false,
                                })
                                .flat_map(|(n, _)| self.iter_syn_impl_item(n))
                                .filter_map(|(n, i)| {
                                    if let Some(name) = ItemName::from(i).get_ident_in_name_space()
                                    {
                                        let impl_item_name =
                                            impl_item_path_elements[index_path_element + 1];
                                        (name == impl_item_name || impl_item_name == "*")
                                            .then_some(n)
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            if impl_item_path_elements[index_path_element + 1] == "*" {
                                for impl_item_index in impl_item_indices {
                                    self.process_impl_item_index(
                                        impl_item_index,
                                        &process_option,
                                        impl_options,
                                    )?;
                                }
                            } else {
                                let impl_item_index = get_index_from_collected_impl_item_indices(
                                    impl_item_indices,
                                    true,
                                    impl_item,
                                )?;
                                self.process_impl_item_index(
                                    impl_item_index,
                                    &process_option,
                                    impl_options,
                                )?;
                            }
                            return Ok(());
                        } else {
                            // search in all impl blocks of all crates and modules for impl items with given impl name
                            let impl_item_indices: Vec<NodeIndex> = self
                                .iter_crates()
                                .flat_map(|(n, _, _)| self.iter_syn_items(n))
                                .filter(|(_, i)| match i {
                                    Item::Impl(item_impl) => {
                                        if let ItemName::TypeStringAndNameString(_, name) =
                                            ItemName::from(*i)
                                        {
                                            item_impl.trait_.is_none() && name == *path_element
                                        } else {
                                            false
                                        }
                                    }
                                    _ => false,
                                })
                                .flat_map(|(n, _)| self.iter_syn_impl_item(n))
                                .filter_map(|(n, i)| {
                                    if let Some(name) = ItemName::from(i).get_ident_in_name_space()
                                    {
                                        let impl_item_name =
                                            impl_item_path_elements[index_path_element + 1];
                                        (name == impl_item_name || impl_item_name == "*")
                                            .then_some(n)
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            if impl_item_path_elements[index_path_element + 1] == "*" {
                                for impl_item_index in impl_item_indices {
                                    self.process_impl_item_index(
                                        impl_item_index,
                                        &process_option,
                                        impl_options,
                                    )?;
                                }
                            } else {
                                let impl_item_index = get_index_from_collected_impl_item_indices(
                                    impl_item_indices,
                                    true,
                                    impl_item,
                                )?;
                                self.process_impl_item_index(
                                    impl_item_index,
                                    &process_option,
                                    impl_options,
                                )?;
                            }
                            return Ok(());
                        }
                    }
                    NameParsingState::ImplItem => {
                        if path_element == "*" {
                            return Err(ChallengeTreeError::NotUniqueImplItemOfConfig(
                                impl_item.to_owned(),
                            ));
                        }
                        // search in all impl blocks of all crates and modules for impl item with given impl name
                        let impl_item_indices: Vec<NodeIndex> = self
                            .iter_crates()
                            .flat_map(|(n, _, _)| self.iter_syn_items(n))
                            .filter(|(_, i)| match i {
                                Item::Impl(item_impl) => item_impl.trait_.is_none(),
                                _ => false,
                            })
                            .flat_map(|(n, _)| self.iter_syn_impl_item(n))
                            .filter_map(|(n, i)| {
                                if let Some(name) = ItemName::from(i).get_ident_in_name_space() {
                                    (name == path_element).then_some(n)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        let impl_item_index = get_index_from_collected_impl_item_indices(
                            impl_item_indices,
                            true,
                            impl_item,
                        )?;
                        self.process_impl_item_index(
                            impl_item_index,
                            &process_option,
                            impl_options,
                        )?;
                        return Ok(());
                    }
                }
            }
        }
    }

    fn process_impl_item_index(
        &self,
        impl_item_index: NodeIndex,
        process_option: &ProcessOption,
        impl_options: &mut HashMap<NodeIndex, bool>,
    ) -> TreeResult<()> {
        match process_option {
            ProcessOption::Include => {
                if self.options.verbose() {
                    println!(
                        "Setting include option for '{}'.",
                        self.get_verbose_name_of_tree_node(impl_item_index)?
                    );
                }
                impl_options.insert(impl_item_index, true);
            }
            ProcessOption::Exclude => {
                if let Entry::Vacant(entry) = impl_options.entry(impl_item_index) {
                    if self.options.verbose() {
                        println!(
                            "Setting exclude option for '{}'.",
                            self.get_verbose_name_of_tree_node(impl_item_index)?
                        );
                    }
                    entry.insert(false);
                }
            }
        }
        Ok(())
    }
}

fn get_index_from_collected_impl_item_indices(
    impl_item_indices: Vec<NodeIndex>,
    module_index_exists: bool,
    impl_item: &str,
) -> TreeResult<NodeIndex> {
    match impl_item_indices.len() {
        0 => Err(ChallengeTreeError::NotExistingImplItemOfConfig(
            impl_item.to_owned(),
        )),
        1 => Ok(impl_item_indices[0]),
        2.. => {
            if module_index_exists {
                Err(ChallengeTreeError::NotUniqueImplItemPossible(
                    impl_item.to_owned(),
                ))
            } else {
                Err(ChallengeTreeError::NotUniqueImplItemOfConfig(
                    impl_item.to_owned(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use syn::Item;

    use crate::parsing::ItemName;

    use super::super::super::processing::tests::setup_processing_test;

    #[test]
    fn test_read_impl_item_options() {
        // preparation
        let mut cg_data = setup_processing_test()
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
        let include_items: Vec<String> = vec!["apply_action".into(), "MyMap2D::set".into()];
        let exclude_items: Vec<String> = vec![
            "set_black".into(),
            "MyMap2D::get".into(),
            "my_map_point::MapPoint::delta_xy".into(),
            "my_map_two_dim::my_map_point::MapPoint::map_position".into(),
            "Compass::*".into(),
        ];
        cg_data.options.set_impl_include(include_items);
        cg_data.options.set_impl_exclude(exclude_items);
        cg_data
            .options
            .set_impl_item_toml("./test/text_impl_config.toml".into());
        let mapping = cg_data.map_impl_config_options_to_node_indices().unwrap();

        // check impl items of my_map_two_dim
        let (my_map_two_dim_crate_index, _, _) = cg_data
            .iter_crates()
            .find(|(_, _, cf)| cf.name == "my_map_two_dim")
            .unwrap();
        let (my_map_two_dim_impl_index, _) = cg_data
            .iter_syn_item_neighbors(my_map_two_dim_crate_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_none(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::TypeStringAndNameString(_, name) = ItemName::from(*i) {
                    name == "MyMap2D"
                } else {
                    false
                }
            })
            .unwrap();
        let (my_map_two_dim_get_index, _) = cg_data
            .iter_syn_impl_item(my_map_two_dim_impl_index)
            .find(|(_, i)| {
                if let Some(id) = ItemName::from(*i).get_ident_in_name_space() {
                    id == "get"
                } else {
                    false
                }
            })
            .unwrap();
        assert_eq!(mapping.get(&my_map_two_dim_get_index), Some(&false));
        let (my_map_two_dim_set_index, _) = cg_data
            .iter_syn_impl_item(my_map_two_dim_impl_index)
            .find(|(_, i)| {
                if let Some(id) = ItemName::from(*i).get_ident_in_name_space() {
                    id == "set"
                } else {
                    false
                }
            })
            .unwrap();
        assert_eq!(mapping.get(&my_map_two_dim_set_index), Some(&true));

        // check impl items of my_map_two_dim
        let (my_map_point_module_index, _) = cg_data
            .iter_syn_item_neighbors(my_map_two_dim_crate_index)
            .filter(|(_, i)| matches!(i, Item::Mod(_)))
            .find(|(_, i)| {
                if let Some(name) = ItemName::from(*i).get_ident_in_name_space() {
                    name == "my_map_point"
                } else {
                    false
                }
            })
            .unwrap();
        let (my_map_point_impl_index, _) = cg_data
            .iter_syn_item_neighbors(my_map_point_module_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_none(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::TypeStringAndNameString(_, name) = ItemName::from(*i) {
                    name == "MapPoint"
                } else {
                    false
                }
            })
            .unwrap();
        let (my_map_point_delta_xy_index, _) = cg_data
            .iter_syn_impl_item(my_map_point_impl_index)
            .find(|(_, i)| {
                if let Some(id) = ItemName::from(*i).get_ident_in_name_space() {
                    id == "delta_xy"
                } else {
                    false
                }
            })
            .unwrap();
        assert_eq!(mapping.get(&my_map_point_delta_xy_index), Some(&false));
        let (my_map_point_map_position_index, _) = cg_data
            .iter_syn_impl_item(my_map_point_impl_index)
            .find(|(_, i)| {
                if let Some(id) = ItemName::from(*i).get_ident_in_name_space() {
                    id == "map_position"
                } else {
                    false
                }
            })
            .unwrap();
        assert_eq!(mapping.get(&my_map_point_map_position_index), Some(&false));

        // check impl items of cg_fusion_binary_test
        let (cg_fusion_binary_test_lib_crate_index, _, _) = cg_data
            .iter_crates()
            .find(|(_, lib, cf)| *lib && cf.name == "cg_fusion_binary_test")
            .unwrap();
        let (go_impl_index, _) = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_lib_crate_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_none(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::TypeStringAndNameString(_, name) = ItemName::from(*i) {
                    name == "Go"
                } else {
                    false
                }
            })
            .unwrap();
        let (go_apply_action_index, _) = cg_data
            .iter_syn_impl_item(go_impl_index)
            .find(|(_, i)| {
                if let Some(id) = ItemName::from(*i).get_ident_in_name_space() {
                    id == "apply_action"
                } else {
                    false
                }
            })
            .unwrap();
        assert_eq!(mapping.get(&go_apply_action_index), Some(&true));

        // check impl items of action
        let (action_module_index, _) = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_lib_crate_index)
            .filter(|(_, i)| matches!(i, Item::Mod(_)))
            .find(|(_, i)| {
                if let Some(name) = ItemName::from(*i).get_ident_in_name_space() {
                    name == "action"
                } else {
                    false
                }
            })
            .unwrap();
        let (action_impl_index, _) = cg_data
            .iter_syn_item_neighbors(action_module_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_none(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::TypeStringAndNameString(_, name) = ItemName::from(*i) {
                    name == "Action"
                } else {
                    false
                }
            })
            .unwrap();
        let (action_set_black_index, _) = cg_data
            .iter_syn_impl_item(action_impl_index)
            .find(|(_, i)| {
                if let Some(id) = ItemName::from(*i).get_ident_in_name_space() {
                    id == "set_black"
                } else {
                    false
                }
            })
            .unwrap();
        assert_eq!(mapping.get(&action_set_black_index), Some(&false));

        // check impl items of Compass
        let (my_compass_module_index, _) = cg_data
            .iter_syn_item_neighbors(my_map_point_module_index)
            .filter(|(_, i)| matches!(i, Item::Mod(_)))
            .find(|(_, i)| {
                if let Some(name) = ItemName::from(*i).get_ident_in_name_space() {
                    name == "my_compass"
                } else {
                    false
                }
            })
            .unwrap();
        let (compass_impl_index, _) = cg_data
            .iter_syn_item_neighbors(my_compass_module_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_none(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::TypeStringAndNameString(_, name) = ItemName::from(*i) {
                    name == "Compass"
                } else {
                    false
                }
            })
            .unwrap();
        for (compass_impl_item_index, _) in cg_data.iter_syn_impl_item(compass_impl_index) {
            assert_eq!(mapping.get(&compass_impl_item_index), Some(&false));
        }

        // check impl items of my_array
        let (my_array_crate_index, _, _) = cg_data
            .iter_crates()
            .find(|(_, _, cf)| cf.name == "my_array")
            .unwrap();
        let (my_array_impl_index, _) = cg_data
            .iter_syn_item_neighbors(my_array_crate_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_none(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::TypeStringAndNameString(_, name) = ItemName::from(*i) {
                    name == "MyArray"
                } else {
                    false
                }
            })
            .unwrap();
        for (my_array_impl_item_index, impl_item) in cg_data.iter_syn_impl_item(my_array_impl_index)
        {
            let item_name = ItemName::from(impl_item).get_ident_in_name_space().unwrap();
            assert_eq!(
                mapping.get(&my_array_impl_item_index),
                Some(&(item_name == "new" || item_name == "set"))
            );
        }
    }
}

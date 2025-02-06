// fn to map impl options from config to node indices of impl items

use super::{ChallengeTreeError, EdgeType, TreeResult};

use crate::parsing::ItemName;
use crate::{
    add_context,
    configuration::CgCli,
    utilities::{current_dir_utf8, get_relative_path, is_inside_dir},
    CgData,
};
use anyhow::{anyhow, Context};
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::stable_graph::NodeIndex;
use serde::Deserialize;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fs;
use syn::Item;

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ImplOptions {
    pub include_impl_items: Vec<String>,
    pub exclude_impl_items: Vec<String>,
}

#[derive(Debug)]
enum ProcessOption {
    Include,
    Exclude,
}

impl From<bool> for ProcessOption {
    fn from(value: bool) -> Self {
        if value {
            ProcessOption::Include
        } else {
            ProcessOption::Exclude
        }
    }
}

impl From<&bool> for ProcessOption {
    fn from(value: &bool) -> Self {
        ProcessOption::from(*value)
    }
}

impl<O: CgCli, S> CgData<O, S> {
    pub(crate) fn get_impl_config_toml_path(&self) -> TreeResult<Option<Utf8PathBuf>> {
        if let Some(ref toml_config_path) = self.options.processing().impl_item_toml {
            let toml_config_path = Utf8PathBuf::try_from(toml_config_path.to_owned())?;
            self.verify_path_points_inside_challenge_dir(&toml_config_path)?;
            let current_dir = current_dir_utf8()?;
            let relative_toml_config_path = get_relative_path(&current_dir, &toml_config_path)?;
            Ok(Some(relative_toml_config_path))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn verify_path_points_inside_challenge_dir(
        &self,
        path: &Utf8PathBuf,
    ) -> TreeResult<()> {
        let challenge_dir = &self.challenge_package().path;
        if !is_inside_dir(&challenge_dir, &path)? {
            return Err(ChallengeTreeError::NotInsideChallengeDir(path.to_owned()));
        }
        Ok(())
    }

    // ToDo: we although need the option to add impl blocks, which will
    // 1.) automatically include all items of the impl block, if it has a trait
    // 2.) only add the block without it items, if it does not have a trait
    pub(crate) fn map_impl_config_options_to_node_indices(
        &self,
    ) -> TreeResult<HashMap<NodeIndex, bool>> {
        let mut impl_options_map: HashMap<NodeIndex, bool> = HashMap::new();
        // load config file if existing
        let impl_config = if let Some(toml_config_path) = self.get_impl_config_toml_path()? {
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
            .processing()
            .include_impl_item
            .iter()
            .chain(impl_config.include_impl_items.iter())
            .map(|ii| (ii, ProcessOption::Include))
            .chain(
                self.options
                    .processing()
                    .exclude_impl_item
                    .iter()
                    .chain(impl_config.exclude_impl_items.iter())
                    .map(|ii| (ii, ProcessOption::Exclude)),
            )
        {
            for impl_item_index in self.collect_impl_config_option_indices(impl_item)? {
                self.process_impl_item_index(
                    impl_item_index,
                    &process_option,
                    &mut impl_options_map,
                )?;
            }
        }
        Ok(impl_options_map)
    }

    pub(crate) fn map_node_indices_to_impl_config_options(
        &self,
        impl_options_map: &HashMap<NodeIndex, bool>,
    ) -> TreeResult<ImplOptions> {
        let mut include_impl_items: Vec<String> = Vec::new();
        let mut exclude_impl_items: Vec<String> = Vec::new();
        for (item_index, process_option) in impl_options_map
            .iter()
            .map(|(k, v)| (k, ProcessOption::from(v)))
        {
            let mut impl_item_path = if let Some(impl_item) = self.get_syn_impl_item(*item_index) {
                ItemName::from(impl_item)
                    .get_ident_in_name_space()
                    .context(add_context!("Expected impl item ident in name space"))?
                    .to_string()
            } else {
                return Err(anyhow!("{}", add_context!("Expected impl item")).into());
            };
            let reverse_check = match self.collect_impl_config_option_indices(&impl_item_path) {
                Ok(impl_item) => impl_item,
                Err(_) => {
                    let impl_block_node = self
                        .get_parent_index_by_edge_type(*item_index, EdgeType::Syn)
                        .context(add_context!("Expected node of impl block."))?;
                    if let Some(item) = self.get_syn_item(impl_block_node) {
                        if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(item) {
                            impl_item_path = format!("{}::{}", impl_block, impl_item_path);
                        } else {
                            return Err(anyhow!(add_context!("Expected impl block name")).into());
                        }
                    } else {
                        return Err(anyhow!(add_context!("Expected impl block item")).into());
                    };
                    self.collect_impl_config_option_indices(&impl_item_path)?
                }
            };
            assert_eq!(*item_index, reverse_check[0]);
            match process_option {
                ProcessOption::Include => include_impl_items.push(impl_item_path),
                ProcessOption::Exclude => exclude_impl_items.push(impl_item_path),
            }
        }
        include_impl_items.sort();
        exclude_impl_items.sort();
        Ok(ImplOptions {
            include_impl_items,
            exclude_impl_items,
        })
    }

    fn collect_impl_config_option_indices(&self, impl_item: &str) -> TreeResult<Vec<NodeIndex>> {
        let impl_item_path_elements: Vec<&str> = impl_item.split("::").collect();
        let with_fully_qualified_impl_block = match impl_item_path_elements.len() {
            0 => {
                return Err(ChallengeTreeError::InvalidImplConfigOption(
                    impl_item.to_string(),
                ))
            }
            1 => false,
            2 => true,
            3.. => {
                return Err(ChallengeTreeError::InvalidImplConfigOption(
                    impl_item.to_string(),
                ))
            }
        };
        if with_fully_qualified_impl_block {
            // search in all impl blocks of all crates and modules for impl items with given impl name
            let impl_item_indices: Vec<NodeIndex> = self
                .iter_crates()
                .flat_map(|(n, _, _)| self.iter_syn_items(n))
                .filter(|(_, i)| {
                    if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                        impl_block == impl_item_path_elements[0]
                    } else {
                        false
                    }
                })
                .flat_map(|(n, _)| self.iter_syn_impl_item(n))
                .filter_map(|(n, i)| {
                    if let Some(name) = ItemName::from(i).get_ident_in_name_space() {
                        (name == impl_item_path_elements[1] || impl_item_path_elements[1] == "*")
                            .then_some(n)
                    } else {
                        None
                    }
                })
                .collect();
            if impl_item_path_elements[1] == "*" || impl_item_indices.len() == 1 {
                Ok(impl_item_indices)
            } else if impl_item_indices.is_empty() {
                Err(ChallengeTreeError::NotExistingImplItemOfConfig(
                    impl_item.to_owned(),
                ))
            } else {
                unreachable!(
                    "Name space rules of rust prevent multiple items with the same name \
                     in a fully qualified impl block."
                );
            }
        } else {
            if impl_item_path_elements[0] == "*" {
                return Err(ChallengeTreeError::InvalidImplConfigOption(
                    impl_item.to_owned(),
                ));
            }
            // search in all impl blocks of all crates and modules for impl item with given impl name
            let impl_item_indices: Vec<NodeIndex> = self
                .iter_crates()
                .flat_map(|(n, _, _)| self.iter_syn_items(n))
                .filter(|(_, i)| matches!(i, Item::Impl(_)))
                .flat_map(|(n, _)| self.iter_syn_impl_item(n))
                .filter_map(|(n, i)| {
                    if let Some(name) = ItemName::from(i).get_ident_in_name_space() {
                        (name == impl_item_path_elements[0]).then_some(n)
                    } else {
                        None
                    }
                })
                .collect();
            if impl_item_indices.len() == 1 {
                Ok(impl_item_indices)
            } else if impl_item_indices.is_empty() {
                Err(ChallengeTreeError::NotExistingImplItemOfConfig(
                    impl_item.to_owned(),
                ))
            } else {
                Err(ChallengeTreeError::NotUniqueImplItem(impl_item.to_owned()))
            }
        }
    }

    fn process_impl_item_index(
        &self,
        impl_item_index: NodeIndex,
        process_option: &ProcessOption,
        impl_options_map: &mut HashMap<NodeIndex, bool>,
    ) -> TreeResult<()> {
        match process_option {
            ProcessOption::Include => {
                if self.options.verbose() {
                    println!(
                        "Setting include option for '{}'.",
                        self.get_verbose_name_of_tree_node(impl_item_index)?
                    );
                }
                impl_options_map.insert(impl_item_index, true);
            }
            ProcessOption::Exclude => {
                if let Entry::Vacant(entry) = impl_options_map.entry(impl_item_index) {
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

#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use petgraph::stable_graph::NodeIndex;
    use syn::Item;

    use crate::parsing::ItemName;

    use super::super::super::processing::tests::setup_processing_test;

    #[test]
    fn test_map_impl_config_options_to_node_indices() {
        // preparation
        let mut cg_data = setup_processing_test(false)
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
        let include_items: Vec<String> = vec!["apply_action".into(), "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set".into()];
        let exclude_items: Vec<String> = vec![
            "set_black".into(),
            "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::get".into(),
            "impl<constX:usize,constY:usize> MapPoint<X,Y>::delta_xy".into(),
            "impl<constX:usize,constY:usize> MapPoint<X,Y>::map_position".into(),
            "impl Compass::*".into(),
        ];
        cg_data.options.set_impl_include(include_items);
        cg_data.options.set_impl_exclude(exclude_items);

        cg_data
            .options
            .set_impl_item_toml("../cg_fusion_binary_test/test/test_impl_config.toml".into());
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
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>"
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
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl<constX:usize,constY:usize> MapPoint<X,Y>"
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
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl Go"
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
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl Action"
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
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl Compass"
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
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl<T:Copy+Clone+Default,constN:usize> MyArray<T,N>"
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

    #[test]
    fn test_map_node_indices_to_impl_config_options() {
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

        // get some node indices of impl items
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
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>"
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
        let (cg_fusion_binary_test_lib_crate_index, _, _) = cg_data
            .iter_crates()
            .find(|(_, lib, cf)| *lib && cf.name == "cg_fusion_binary_test")
            .unwrap();
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
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl Action"
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

        let mut mapping: HashMap<NodeIndex, bool> = HashMap::new();
        mapping.insert(my_map_two_dim_get_index, true);
        mapping.insert(my_map_two_dim_set_index, true);
        mapping.insert(action_set_black_index, false);

        // assert
        let mut impl_options = cg_data
            .map_node_indices_to_impl_config_options(&mapping)
            .unwrap();
        impl_options.include_impl_items.sort();
        assert_eq!(
            impl_options.include_impl_items,
            [
                "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::get",
                "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>::set",
            ]
        );
        assert_eq!(impl_options.exclude_impl_items, ["set_black"]);
    }
}

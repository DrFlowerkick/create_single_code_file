// fn to map impl options from config to node indices of impl items

use super::{ChallengeTreeError, EdgeType, TreeResult};

use crate::challenge_tree::NodeType;
use crate::parsing::ItemName;
use crate::{
    CgData, add_context,
    configuration::CgCli,
    utilities::{current_dir_utf8, get_relative_path, is_inside_dir},
};
use anyhow::{Context, anyhow};
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::stable_graph::NodeIndex;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs;
use syn::Item;
use toml_edit::{Array, Value};

#[derive(Debug, Deserialize, Default)]
pub(crate) struct InExClude {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ImplOptions {
    pub impl_items: InExClude,
    pub impl_blocks: InExClude,
}

impl ImplOptions {
    pub(crate) fn impl_items_include_to_toml_array(&self) -> Array {
        convert_vec_string_to_toml_array(&self.impl_items.include)
    }
    pub(crate) fn impl_items_exclude_to_toml_array(&self) -> Array {
        convert_vec_string_to_toml_array(&self.impl_items.exclude)
    }
    pub(crate) fn impl_blocks_include_to_toml_array(&self) -> Array {
        convert_vec_string_to_toml_array(&self.impl_blocks.include)
    }
    pub(crate) fn impl_blocks_exclude_to_toml_array(&self) -> Array {
        convert_vec_string_to_toml_array(&self.impl_blocks.exclude)
    }
}

fn convert_vec_string_to_toml_array(vec_string: &[String]) -> Array {
    let mut toml_array = Array::new();
    for element in vec_string.iter() {
        let formatted = Value::from(element);
        let formatted = formatted.decorated("\n    ", "");
        toml_array.push_formatted(formatted);
    }
    toml_array.set_trailing("\n");
    toml_array
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

enum ImplOptionType {
    Item,
    ItemWithImplBlock,
    Block,
}

impl<O: CgCli, S> CgData<O, S> {
    pub(crate) fn get_impl_config_toml_path(&self) -> TreeResult<Option<Utf8PathBuf>> {
        match self.options.processing().impl_item_toml {
            Some(ref toml_config_path) => {
                let toml_config_path = Utf8PathBuf::try_from(toml_config_path.to_owned())?;
                self.verify_path_points_inside_challenge_dir(&toml_config_path)?;
                let current_dir = current_dir_utf8()?;
                let relative_toml_config_path = get_relative_path(&current_dir, &toml_config_path)?;
                Ok(Some(relative_toml_config_path))
            }
            _ => Ok(None),
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

    pub(crate) fn map_impl_config_options_to_node_indices(
        &self,
    ) -> TreeResult<HashMap<NodeIndex, bool>> {
        let mut impl_options_map: HashMap<NodeIndex, bool> = HashMap::new();
        // load config file if existing
        let impl_config = match self.get_impl_config_toml_path()? {
            Some(toml_config_path) => {
                let toml_string = fs::read_to_string(toml_config_path)?;
                let toml_options: ImplOptions = toml::from_str(&toml_string)?;
                toml_options
            }
            _ => ImplOptions::default(),
        };
        // Collect all impl items to include or exclude.
        // If index is already in include, include wins.
        for (impl_item, process_option) in self
            .options
            .processing()
            .include_impl_item
            .iter()
            .chain(impl_config.impl_items.include.iter())
            .chain(impl_config.impl_blocks.include.iter())
            .map(|ii| (ii, ProcessOption::Include))
            .chain(
                self.options
                    .processing()
                    .exclude_impl_item
                    .iter()
                    .chain(impl_config.impl_items.exclude.iter())
                    .chain(impl_config.impl_blocks.exclude.iter())
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
        let mut include_impl_blocks: Vec<String> = Vec::new();
        let mut exclude_impl_blocks: Vec<String> = Vec::new();
        for (item_index, process_option) in impl_options_map
            .iter()
            .map(|(k, v)| (k, ProcessOption::from(v)))
        {
            match self.tree.node_weight(*item_index) {
                Some(NodeType::SynItem(Item::Impl(item_impl))) => {
                    let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(item_impl)
                    else {
                        return Err(anyhow!(add_context!("Expected impl block name")).into());
                    };
                    let reverse_check = self.collect_impl_config_option_indices(&impl_block)?;
                    // impl blocks of the same name may be used multiple times in the same crate
                    assert_eq!(reverse_check.iter().find(|rc| *rc == item_index), Some(item_index));
                    match process_option {
                        ProcessOption::Include => include_impl_blocks.push(impl_block),
                        ProcessOption::Exclude => exclude_impl_blocks.push(impl_block),
                    }
                }
                Some(NodeType::SynImplItem(impl_item)) => {
                    let mut impl_item_path = ItemName::from(impl_item)
                        .get_ident_in_name_space()
                        .context(add_context!("Expected impl item ident in name space"))?
                        .to_string();
                    let reverse_check = match self
                        .collect_impl_config_option_indices(&impl_item_path)
                    {
                        Ok(impl_item_node) => impl_item_node,
                        Err(_) => {
                            let impl_block_node = self
                                .get_parent_index_by_edge_type(*item_index, EdgeType::Syn)
                                .context(add_context!("Expected node of impl block."))?;
                            if let Some(item) = self.get_syn_item(impl_block_node) {
                                if let ItemName::ImplBlockIdentifier(impl_block) =
                                    ItemName::from(item)
                                {
                                    impl_item_path = format!("{}@{}", impl_item_path, impl_block);
                                } else {
                                    return Err(
                                        anyhow!(add_context!("Expected impl block name")).into()
                                    );
                                }
                            } else {
                                return Err(
                                    anyhow!(add_context!("Expected impl block item")).into()
                                );
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
                _ => return Err(anyhow!("{}", add_context!("Expected impl item or block.")).into()),
            }
        }
        include_impl_items.sort();
        exclude_impl_items.sort();
        include_impl_blocks.sort();
        exclude_impl_blocks.sort();
        Ok(ImplOptions {
            impl_items: InExClude {
                include: include_impl_items,
                exclude: exclude_impl_items,
            },
            impl_blocks: InExClude {
                include: include_impl_blocks,
                exclude: exclude_impl_blocks,
            },
        })
    }

    fn collect_impl_config_option_indices(&self, impl_item: &str) -> TreeResult<Vec<NodeIndex>> {
        let impl_item_path_elements: Vec<&str> = impl_item.split("@").collect();
        let with_fully_qualified_impl_block = match impl_item_path_elements.len() {
            0 => {
                return Err(ChallengeTreeError::InvalidImplConfigOption(
                    impl_item.to_string(),
                ));
            }
            1 => {
                // functions do not contain whitespace, but every impl name contains at least one whitespace
                if impl_item.chars().any(|c| c.is_whitespace()) {
                    ImplOptionType::Block
                } else {
                    ImplOptionType::Item
                }
            }
            2 => ImplOptionType::ItemWithImplBlock,
            3.. => {
                return Err(ChallengeTreeError::InvalidImplConfigOption(
                    impl_item.to_string(),
                ));
            }
        };
        match with_fully_qualified_impl_block {
            ImplOptionType::ItemWithImplBlock => {
                // search in all impl blocks of all crates and modules for impl items with given impl name
                let impl_item_indices: Vec<NodeIndex> = self
                    .iter_crates()
                    .flat_map(|(n, _, _)| self.iter_syn_items(n))
                    .filter(|(_, i)| {
                        if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                            impl_block == impl_item_path_elements[1]
                        } else {
                            false
                        }
                    })
                    .flat_map(|(n, _)| self.iter_syn_impl_item(n))
                    .filter_map(|(n, i)| {
                        if let Some(name) = ItemName::from(i).get_ident_in_name_space() {
                            (name == impl_item_path_elements[0]
                                || impl_item_path_elements[0] == "*")
                                .then_some(n)
                        } else {
                            None
                        }
                    })
                    .collect();
                if impl_item_path_elements[0] == "*" || impl_item_indices.len() == 1 {
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
            }
            ImplOptionType::Item => {
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
            ImplOptionType::Block => {
                // search all impl blocks; it is possible to have multiple impl blocks with the same
                // fully qualified name, e.g. 'impl TypeFoo' may be used multiple times
                let impl_block_indices: Vec<NodeIndex> = self
                    .iter_crates()
                    .flat_map(|(n, _, _)| self.iter_syn_items(n))
                    .filter_map(|(n, i)| {
                        if let ItemName::ImplBlockIdentifier(name) = ItemName::from(i) {
                            (name == impl_item_path_elements[0]).then_some(n)
                        } else {
                            None
                        }
                    })
                    .collect();
                if impl_block_indices.is_empty() {
                    Err(ChallengeTreeError::NotExistingImplItemOfConfig(
                        impl_item.to_owned(),
                    ))
                } else {
                    Ok(impl_block_indices)
                }
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

        let include_items: Vec<String> = vec![
            "apply_action".into(),
            "set@impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>".into(),
            "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>".into(),
        ];
        let exclude_items: Vec<String> = vec![
            "set_black".into(),
            "impl Display for Action".into(),
            "get@impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>".into(),
            "*@impl Compass".into(),
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
        let (my_map_two_dim_impl_default_index, _) = cg_data
            .iter_syn_item_neighbors(my_map_two_dim_crate_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_some(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>"
                } else {
                    false
                }
            })
            .unwrap();
        assert_eq!(mapping.get(&my_map_two_dim_impl_default_index), Some(&true));
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
        let (action_impl_display_index, _) = cg_data
            .iter_syn_item_neighbors(action_module_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_some(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl Display for Action"
                } else {
                    false
                }
            })
            .unwrap();
        assert_eq!(mapping.get(&action_impl_display_index), Some(&false));
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
        let (my_array_impl_default_index, _) = cg_data
            .iter_syn_item_neighbors(my_array_crate_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_some(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl<T:Copy+Clone+Default,constN:usize> Default for MyArray<T,N>"
                } else {
                    false
                }
            })
            .unwrap();
        assert_eq!(mapping.get(&my_array_impl_default_index), Some(&true));
        let (my_array_impl_from_iterator_index, _) = cg_data
            .iter_syn_item_neighbors(my_array_crate_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_some(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl<T,constN:usize> FromIterator<T> for MyArray<T,N> whereT:Copy+Clone+Default,"
                } else {
                    false
                }
            })
            .unwrap();
        assert_eq!(
            mapping.get(&my_array_impl_from_iterator_index),
            Some(&false)
        );
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
        let (my_map_two_dim_impl_default_index, _) = cg_data
            .iter_syn_item_neighbors(my_map_two_dim_crate_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_some(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>"
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
        let (action_impl_display_index, _) = cg_data
            .iter_syn_item_neighbors(action_module_index)
            .filter(|(_, i)| match i {
                Item::Impl(item_impl) => item_impl.trait_.is_some(),
                _ => false,
            })
            .find(|(_, i)| {
                if let ItemName::ImplBlockIdentifier(impl_block) = ItemName::from(*i) {
                    impl_block == "impl Display for Action"
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
        mapping.insert(my_map_two_dim_impl_default_index, true);
        mapping.insert(action_impl_display_index, false);

        // assert
        let mut impl_options = cg_data
            .map_node_indices_to_impl_config_options(&mapping)
            .unwrap();
        impl_options.impl_items.include.sort();
        assert_eq!(
            impl_options.impl_items.include,
            [
                "get@impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>",
                "set@impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> MyMap2D<T,X,Y,N>",
            ]
        );
        assert_eq!(impl_options.impl_items.exclude, ["set_black"]);
        assert_eq!(
            impl_options.impl_blocks.include,
            [
                "impl<T:Copy+Clone+Default,constX:usize,constY:usize,constN:usize> Default for MyMap2D<T,X,Y,N>"
            ]
        );
        assert_eq!(
            impl_options.impl_blocks.exclude,
            ["impl Display for Action"]
        );
    }
}

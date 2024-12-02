// functions to analyze use statements in src files

use super::AnalyzeState;
use crate::{
    add_context,
    challenge_tree::NodeTyp,
    configuration::CliInput,
    error::CgResult,
    parsing::get_use_items,
    utilities::{is_pascal_case, is_shouty_snake_case, is_snake_case},
    CgData,
};
use anyhow::Context;
use petgraph::graph::NodeIndex;
use syn::{Ident, Item, UseName, UseRename};

#[derive(Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
enum UseItemTypes {
    PascalCase(Ident),
    SnakeCase(Ident),
    ShoutySnakeCase(Ident),
    PascalCaseRename(Ident, Ident),
    SnakeCaseRename(Ident, Ident),
    ShoutySnakeCaseRename(Ident, Ident),
    _Asterisk,
    Unknown,
}

impl From<UseName> for UseItemTypes {
    fn from(value: UseName) -> Self {
        let name = value.ident.to_string();
        match (
            is_pascal_case(&name),
            is_snake_case(&name),
            is_shouty_snake_case(&name),
        ) {
            (true, _, _) => UseItemTypes::PascalCase(value.ident),
            (_, true, _) => UseItemTypes::SnakeCase(value.ident),
            (_, _, true) => UseItemTypes::ShoutySnakeCase(value.ident),
            _ => UseItemTypes::Unknown,
        }
    }
}

impl From<UseRename> for UseItemTypes {
    fn from(value: UseRename) -> Self {
        let name = value.ident.to_string();
        match (
            is_pascal_case(&name),
            is_snake_case(&name),
            is_shouty_snake_case(&name),
        ) {
            (true, _, _) => UseItemTypes::PascalCaseRename(value.ident, value.rename),
            (_, true, _) => UseItemTypes::SnakeCaseRename(value.ident, value.rename),
            (_, _, true) => UseItemTypes::ShoutySnakeCaseRename(value.ident, value.rename),
            _ => UseItemTypes::Unknown,
        }
    }
}

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn expand_use_statements(&mut self) -> CgResult<()> {
        // get challenge bin and all lib crate indices
        let (bin_crate_index, _) = self
            .get_challenge_bin_crate()
            .context(add_context!("expected challenge bin."))?;
        let mut crate_indices: Vec<NodeIndex> = self.iter_lib_crates().map(|(n, _)| n).collect();
        crate_indices.push(bin_crate_index);
        for crate_index in crate_indices {
            // get indices of SynItem Nodes, which contain UseItems
            let syn_use_indices: Vec<NodeIndex> = self
                .iter_syn_items(crate_index)
                .filter_map(|(n, i)| if let Item::Use(_) = i { Some(n) } else { None })
                .collect();
            for syn_use_index in syn_use_indices {
                // get source (parent) of syn use item
                let source_index = self
                    .get_syn_item_source_index(syn_use_index)
                    .context(add_context!("Expected source index of syn item."))?;
                // remove old use item from tree
                let old_use_item = self
                    .tree
                    .remove_node(syn_use_index)
                    .context(add_context!("Expected syn node to remove"))?;
                // expand and collect use items and add them to tree
                if let NodeTyp::SynItem(Item::Use(use_item)) = old_use_item {
                    for new_use_tree in get_use_items(&use_item.tree) {
                        let mut new_use_item = use_item.to_owned();
                        new_use_item.tree = new_use_tree;
                        self.add_syn_item(&Item::Use(new_use_item), &"".into(), source_index)?;
                    }
                } else {
                    unreachable!("Node to remove must be SynItem of Item::Use");
                }
            }
        }
        Ok(())
    }
    /*
    pub fn analyze_use_statements(&self) -> CgResult<()> {
        let external_dependencies: Vec<String> = self
            .iter_external_dependencies()
            .map(|(_, n)| n.to_string())
            .collect();
        let (bin_index, bin_crate) = self
            .get_challenge_bin_crate()
            .context(add_context!("Expected challenge bin crate."))?;
        let mut use_visitor = UseVisitor::new(external_dependencies);
        use_visitor.visit_file(&bin_crate.syntax.borrow());
        let mut use_items: BTreeSet<(NodeIndex, UseItemTypes)> = BTreeSet::new();
        for item_use in use_visitor.uses.iter() {
            self.parse_use_item(&item_use.tree, bin_index, bin_index, &mut use_items)?;
        }
        Ok(())
    }

    fn parse_use_item(
        &self,
        use_tree: &UseTree,
        current_index: NodeIndex,
        crate_index: NodeIndex,
        use_items: &mut BTreeSet<(NodeIndex, UseItemTypes)>,
    ) -> CgResult<()> {
        match use_tree {
            UseTree::Path(use_path) => {
                let module = use_path.ident.to_string();
                match module.as_str() {
                    "crate" => {
                        // module of current crate
                        self.parse_use_item(&use_path.tree, crate_index, crate_index, use_items)?;
                    }
                    "self" => {
                        // current module
                        self.parse_use_item(&use_path.tree, current_index, crate_index, use_items)?;
                    }
                    "super" => {
                        // parent module
                        let parent_index = self
                            .tree
                            .edges_directed(current_index, Direction::Incoming)
                            .filter(|e| *e.weight() == EdgeType::Module)
                            .map(|e| e.source())
                            .next()
                            .context(add_context!(
                                "Expected one 'Module' edge pointing to current node."
                            ))?;
                        self.parse_use_item(&use_path.tree, parent_index, crate_index, use_items)?;
                    }
                    _ => {
                        // some module, could be module of current file or local package dependency
                        if let Some((lib_crate_index, _)) =
                            self.iter_lib_crates().find(|(_, cf)| cf.name == module)
                        {
                            self.parse_use_item(
                                &use_path.tree,
                                lib_crate_index,
                                lib_crate_index,
                                use_items,
                            )?;
                        } else {
                            let (module_index, _) = self
                                .iter_modules(current_index)?
                                .find(|(_, m)| m.name == module)
                                .context(add_context!(format!(
                                    "Expected sub module '{module} at index '{:?}'",
                                    current_index
                                )))?;
                            self.parse_use_item(
                                &use_path.tree,
                                module_index,
                                crate_index,
                                use_items,
                            )?;
                        }
                    }
                }
            }
            UseTree::Group(use_group) => {
                for group_item in use_group.items.iter() {
                    self.parse_use_item(group_item, current_index, crate_index, use_items)?;
                }
            }
            UseTree::Glob(_) => {
                use_items.insert((current_index, UseItemTypes::Asterisk));
            }
            UseTree::Name(use_name) => {
                let use_item_type = UseItemTypes::from(use_name.to_owned());
                use_items.insert((current_index, use_item_type));
            }
            UseTree::Rename(use_rename) => {
                let use_item_type = UseItemTypes::from(use_rename.to_owned());
                use_items.insert((current_index, use_item_type));
            }
        }
        Ok(())
    }
     */
}

#[cfg(test)]
mod tests {

    use super::super::tests::setup_analyze_test;
    use super::*;
    use quote::ToTokens;

    #[test]
    fn test_collecting_modules() {
        // preparation
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        cg_data.add_lib_src_files().unwrap();

        // number of use statements before expansion in challenge bin crate
        let (challenge_bin_crate_index, _) = cg_data.get_challenge_bin_crate().unwrap();
        assert_eq!(
            cg_data
                .iter_syn_items(challenge_bin_crate_index)
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
                .iter_syn_items(cg_fusion_lib_test_index)
                .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
                .count(),
            5
        );

        // action to test
        cg_data.expand_use_statements().unwrap();

        // number of use statements after expansion in challenge bin crate
        let (challenge_bin_crate_index, _) = cg_data.get_challenge_bin_crate().unwrap();
        assert_eq!(
            cg_data
                .iter_syn_items(challenge_bin_crate_index)
                .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
                .count(),
            5
        );
        let use_statements: Vec<String> = cg_data
            .iter_syn_items(challenge_bin_crate_index)
            .filter_map(|(_, i)| match i {
                Item::Use(use_item) => Some(use_item.to_token_stream().to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(
            use_statements,
            vec![
                "use cg_fusion_binary_test :: action :: Action ;",
                "use cg_fusion_binary_test :: Y ;",
                "use cg_fusion_binary_test :: X ;",
                "use cg_fusion_binary_test :: Go ;",
                "use cg_fusion_lib_test :: my_map_two_dim :: my_map_point :: * ;"
            ]
        );
        // number of use statements after expansion in cg_fusion_lib_test lib crate
        let (cg_fusion_lib_test_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "cg_fusion_lib_test")
            .unwrap();
        assert_eq!(
            cg_data
                .iter_syn_items(cg_fusion_lib_test_index)
                .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
                .count(),
            6
        );
    }
}

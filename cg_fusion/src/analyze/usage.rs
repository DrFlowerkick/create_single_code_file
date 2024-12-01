// functions to analyze use statements in src files

use super::AnalyzeState;
use crate::{
    add_context,
    challenge_tree::EdgeType,
    configuration::CliInput,
    error::CgResult,
    parsing::UseVisitor,
    utilities::{is_pascal_case, is_shouty_snake_case, is_snake_case},
    CgData,
};
use anyhow::Context;
use petgraph::{graph::NodeIndex, visit::EdgeRef, Direction};
use std::collections::BTreeSet;
use syn::{visit::Visit, Ident, UseName, UseRename, UseTree};

#[derive(Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
enum UseItemTypes {
    PascalCase(Ident),
    SnakeCase(Ident),
    ShoutySnakeCase(Ident),
    PascalCaseRename(Ident, Ident),
    SnakeCaseRename(Ident, Ident),
    ShoutySnakeCaseRename(Ident, Ident),
    Asterisk,
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
}

#[cfg(test)]
mod tests {

    use super::super::tests::setup_analyze_test;
    //use super::*;

    #[test]
    fn test_collecting_modules() {
        // preparation
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        cg_data.add_lib_src_files().unwrap();

        // action to test
        cg_data.analyze_use_statements().unwrap();
    }
}

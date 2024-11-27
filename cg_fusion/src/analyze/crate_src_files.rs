// functions to to add src files of bin and lib crates to tree

use super::{AnalyzeError, AnalyzeState};
use crate::{add_context, configuration::CliInput, error::CgResult, parsing::ModVisitor, CgData};
use anyhow::{anyhow, Context};
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::graph::NodeIndex;
use syn::{visit::Visit, File};

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn add_bin_src_files_of_challenge(&mut self) -> CgResult<()> {
        // get bin name
        let bin_name = self.get_challenge_bin_name();

        // add bin crate to tree
        let bin_crate_index = self.add_binary_crate_to_package(0.into(), bin_name.to_owned())?;
        let binary_crate = self.get_binary_crate(bin_crate_index)?;
        let crate_dir = binary_crate
            .path
            .parent()
            .context(add_context!(
                "Unexpected failure of getting parent of binary crate file."
            ))?
            .to_path_buf();
        let syntax = binary_crate.syntax.borrow().to_owned();
        self.parse_mod_from_src_file(crate_dir, syntax, bin_crate_index, bin_crate_index)?;
        Ok(())
    }

    pub fn add_lib_src_files(&mut self) -> CgResult<()> {
        // collect package indices
        let package_indices: Vec<NodeIndex> = self.iter_local_packages().map(|(n, _)| n).collect();
        for index in package_indices {
            // add library crate to package
            if let Some(lib_crate_index) = self.add_library_crate_to_package(index)? {
                let library_crate = self.get_library_crate(lib_crate_index)?;
                let crate_dir = library_crate
                    .path
                    .parent()
                    .context(add_context!(
                        "Unexpected failure of getting parent of binary crate file."
                    ))?
                    .to_path_buf();
                let syntax = library_crate.syntax.borrow().to_owned();
                self.parse_mod_from_src_file(crate_dir, syntax, lib_crate_index, lib_crate_index)?;
            }
        }
        Ok(())
    }

    fn parse_mod_from_src_file(
        &mut self,
        current_dir: Utf8PathBuf,
        current_syntax: File,
        source_index: NodeIndex,
        crate_index: NodeIndex,
    ) -> Result<(), AnalyzeError> {
        // if current_dir does not exist, it cannot contain further modules.
        // therefore no parsing required
        if !current_dir.is_dir() {
            return Ok(());
        }

        // create visitor from source code
        let mut visitor = ModVisitor::default();
        visitor.visit_file(&current_syntax);

        // parse mod entries, which are empty
        for item_mod in visitor.mods.iter().filter(|m| m.content.is_none()) {
            let module = item_mod.ident.to_string();
            // set module directory
            let mod_dir = current_dir.join(module.as_str());
            // set module filename
            let mut path = mod_dir.join("mod.rs");
            // module is either 'module_name.rs' or 'module_name/mod.rs'
            if !path.is_file() {
                path = mod_dir.clone();
                path.set_extension("rs");
                if !path.is_file() {
                    Err(anyhow!(add_context!("Unexpected module file path error.")))?;
                }
            }
            // add module to tree
            let module_node_index = self.add_module(module, path, source_index, crate_index)?;
            let mod_syntax = self.get_module(module_node_index)?.syntax.borrow().clone();
            // recursive parse for further modules
            self.parse_mod_from_src_file(mod_dir, mod_syntax, module_node_index, crate_index)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::super::tests::setup_analyze_test;
    use super::*;

    #[test]
    fn test_collecting_modules() {
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();

        cg_data.add_bin_src_files_of_challenge().unwrap();
        let (bcf_index, bcf) = cg_data.get_challenge_bin_crate().unwrap();
        assert_eq!(bcf.name, "cg_fusion_binary_test");
        assert_eq!(cg_data.iter_modules(bcf_index).count(), 0);

        cg_data.add_lib_src_files().unwrap();
        let (lcf_index, lcf) = cg_data.get_challenge_lib_crate().unwrap();
        assert_eq!(lcf.name, "cg_fusion_binary_test");
        let (challenge_lib_module_indices, challenge_lib_modules): (Vec<NodeIndex>, Vec<String>) =
            cg_data
                .iter_modules(lcf_index)
                .map(|(n, m)| (n, m.name.to_owned()))
                .unzip();
        assert_eq!(challenge_lib_modules, &["action"]);
        let crate_index = cg_data
            .get_module(challenge_lib_module_indices[0])
            .unwrap()
            .crate_index;
        assert_eq!(crate_index, lcf_index);

        let (dependency_lib_crate_indices, dependency_lib_crates): (Vec<NodeIndex>, Vec<String>) =
            cg_data
                .iter_dependencies_lib_crates()
                .map(|(n, cf)| (n, cf.name.to_owned()))
                .unzip();
        assert_eq!(
            dependency_lib_crates,
            &["cg_fusion_lib_test", "my_map_two_dim", "my_array"]
        );
        assert_eq!(
            cg_data
                .iter_modules(dependency_lib_crate_indices[0])
                .count(),
            0
        );
        assert_eq!(
            cg_data
                .iter_modules(dependency_lib_crate_indices[2])
                .count(),
            0
        );
        let my_map_two_dim_modules: Vec<String> = cg_data
            .iter_modules(dependency_lib_crate_indices[1])
            .map(|(_, m)| m.name.to_owned())
            .collect();
        assert_eq!(my_map_two_dim_modules, &["my_map_point", "my_compass"]);
        assert_eq!(
            cg_data
                .iter_modules(dependency_lib_crate_indices[1])
                .filter(|(_, m)| m.crate_index == dependency_lib_crate_indices[1])
                .count(),
            2
        );
    }
}

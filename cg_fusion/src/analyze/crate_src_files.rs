// functions to to add src files of bin and lib crates to tree

use super::AnalyzeState;
use crate::{add_context, configuration::CgCli, error::CgResult, parsing::load_syntax, CgData};
use anyhow::Context;
use petgraph::graph::NodeIndex;

impl<O: CgCli> CgData<O, AnalyzeState> {
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
        // add syn items of bin crate to tree
        let syntax = load_syntax(&binary_crate.path)?;
        for item in syntax.items.to_owned().iter() {
            self.add_syn_item(item, &crate_dir, bin_crate_index)?;
        }
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
                // add syn items of lib crate to tree
                let syntax = load_syntax(&library_crate.path)?;
                for item in syntax.items.to_owned().iter() {
                    self.add_syn_item(item, &crate_dir, lib_crate_index)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::super::tests::setup_processing_test;
    use syn::Item;

    #[test]
    fn test_collecting_modules() {
        let mut cg_data = setup_processing_test();
        cg_data.add_challenge_dependencies().unwrap();

        cg_data.add_bin_src_files_of_challenge().unwrap();
        let (bcf_index, bcf) = cg_data.get_challenge_bin_crate().unwrap();
        assert_eq!(bcf.name, "cg_fusion_binary_test");
        assert_eq!(cg_data.iter_syn_item_neighbors(bcf_index).count(), 4);

        cg_data.add_lib_src_files().unwrap();
        let (lcf_index, lcf) = cg_data.get_challenge_lib_crate().unwrap();
        assert_eq!(lcf.name, "cg_fusion_binary_test");
        assert_eq!(cg_data.iter_syn_item_neighbors(lcf_index).count(), 12);

        let mut iter_lib_crates = cg_data.iter_lib_crates();
        iter_lib_crates.next();

        let (index, lib_crate) = iter_lib_crates.next().unwrap();
        assert_eq!(lib_crate.name, "cg_fusion_lib_test");
        assert_eq!(cg_data.iter_syn_item_neighbors(index).count(), 7);

        let (index, lib_crate) = iter_lib_crates.next().unwrap();
        assert_eq!(lib_crate.name, "my_map_two_dim");
        assert_eq!(cg_data.iter_syn_item_neighbors(index).count(), 12);

        let (sub_mod_index, sub_mod) = cg_data
            .iter_syn_item_neighbors(index)
            .filter_map(|(n, i)| {
                if let Item::Mod(item_mod) = i {
                    Some((n, item_mod))
                } else {
                    None
                }
            })
            .next()
            .unwrap();
        assert_eq!(sub_mod.ident.to_string(), "my_map_point");
        assert_eq!(cg_data.iter_syn_item_neighbors(sub_mod_index).count(), 11);

        let (sub_mod_index, sub_mod) = cg_data
            .iter_syn_item_neighbors(sub_mod_index)
            .filter_map(|(n, i)| {
                if let Item::Mod(item_mod) = i {
                    Some((n, item_mod))
                } else {
                    None
                }
            })
            .next()
            .unwrap();
        assert_eq!(sub_mod.ident.to_string(), "my_compass");
        assert_eq!(cg_data.iter_syn_item_neighbors(sub_mod_index).count(), 2);

        let (index, lib_crate) = iter_lib_crates.next().unwrap();
        assert_eq!(lib_crate.name, "my_array");
        assert_eq!(cg_data.iter_syn_item_neighbors(index).count(), 5);

        assert!(iter_lib_crates.next().is_none());
    }
}

// functions to to add src files of bin and lib crates to tree

use super::{AnalyzeError, AnalyzeState};
use crate::{add_context, configuration::CliInput, error::CgResult, parsing::ModVisitor, CgData};
use anyhow::{anyhow, Context};
use cargo_metadata::{camino::Utf8PathBuf, Message};
use petgraph::graph::NodeIndex;
use std::fmt::Write;
use syn::{visit::Visit, File};

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn add_bin_src_files_of_challenge(&mut self) -> CgResult<()> {
        // get bin name
        let bin_name = if self.options.input().input == "main" {
            // if main, use crate name for bin name
            self.challenge_package().name.as_str()
        } else {
            self.options.input().input.as_str()
        };

        if self.options.verbose() {
            println!("Running 'cargo check' for bin challenge code...");
        }

        // run 'cargo check' on bin_name to make sure, that input is ready to be processed
        let output = self
            .challenge_package()
            .metadata
            .run_cargo_check_for_binary_of_root_package(bin_name)?;

        // collect any remaining 'cargo check' messages
        let mut check_messages = String::new();
        for message in Message::parse_stream(&output.stdout[..]) {
            if let Message::CompilerMessage(msg) = message.context(add_context!(
                "Unexpected error of parsing 'cargo check' messages stream."
            ))? {
                if let Some(rendered_msg) = msg.message.rendered {
                    writeln!(&mut check_messages, "{}", rendered_msg).context(add_context!(
                        "Unexpected error while formatting rendered 'cargo check' messages."
                    ))?;
                }
            }
        }
        if !check_messages.is_empty() {
            writeln!(
                &mut check_messages,
                "{}",
                String::from_utf8(output.stderr).context(add_context!(
                    "Unexpected error while converting stderr to string."
                ))?
            )
            .context(add_context!(
                "Unexpected error while combining rendered 'cargo check' messages with stderr."
            ))?;
            Err(AnalyzeError::RemainingCargoCheckMessagesOfInput(
                check_messages,
            ))?;
        }

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

    #[test]
    fn test_collecting_modules() {
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        cg_data.add_bin_src_files_of_challenge().unwrap();
        let (_, bcf) = cg_data.get_challenge_bin_crate().unwrap();
        assert_eq!(bcf.name, "cg_fusion_binary_test");
    }
}

// central library

pub mod analyze;
pub mod challenge_tree;
pub mod configuration;
pub mod error;
pub mod file_generation;
pub mod merge;
pub mod metadata;
pub mod parsing;
pub mod preparation;
pub mod solve_cargo_check;
pub mod utilities;

use analyze::AnalyzeState;
use challenge_tree::{ChallengeTree, LocalPackage, NodeType};
use configuration::{AnalyzeCli, CargoCli, FusionCli, MergeCli, PurgeCli};
use error::{CgError, CgResult};
use metadata::MetadataError;

use petgraph::stable_graph::StableDiGraph;

pub enum CgMode {
    Fusion(CgData<FusionCli, AnalyzeState>),
    Analyze(CgData<AnalyzeCli, AnalyzeState>),
    Merge(CgData<MergeCli, AnalyzeState>),
    Purge(CgData<PurgeCli, AnalyzeState>),
}

pub struct NoOptions;
pub struct NoCommand;

pub struct CgDataBuilder<O, M> {
    options: O,
    metadata_command: M,
}

impl Default for CgDataBuilder<NoOptions, NoCommand> {
    fn default() -> Self {
        Self::new()
    }
}

impl CgDataBuilder<NoOptions, NoCommand> {
    pub fn new() -> Self {
        Self {
            options: NoOptions,
            metadata_command: NoCommand,
        }
    }
}

impl CgDataBuilder<NoOptions, NoCommand> {
    pub fn set_options(self, options: CargoCli) -> CgDataBuilder<CargoCli, NoCommand> {
        CgDataBuilder {
            options,
            metadata_command: NoCommand,
        }
    }
}

impl CgDataBuilder<CargoCli, NoCommand> {
    pub fn set_command(self) -> CgDataBuilder<CargoCli, cargo_metadata::MetadataCommand> {
        CgDataBuilder {
            metadata_command: self.options.metadata_command(),
            options: self.options,
        }
    }
}

impl CgDataBuilder<CargoCli, cargo_metadata::MetadataCommand> {
    pub fn build(self) -> CgResult<CgMode> {
        let metadata = self.metadata_command.exec().map_err(MetadataError::from)?;
        // initialize root node with challenge metadata
        let root_node_value = NodeType::LocalPackage(LocalPackage::try_from(metadata)?);
        let mut tree: ChallengeTree = StableDiGraph::new();
        // root node should have index 0
        assert_eq!(tree.add_node(root_node_value), 0.into());
        match self.options {
            CargoCli::CgFusion(fusion_cli) => Ok(CgMode::Fusion(CgData {
                _state: AnalyzeState,
                options: fusion_cli,
                tree,
            })),
            CargoCli::CgAnalyze(analyze_cli) => Ok(CgMode::Analyze(CgData {
                _state: AnalyzeState,
                options: analyze_cli,
                tree,
            })),
            CargoCli::CgMerge(merge_cli) => Ok(CgMode::Merge(CgData {
                _state: AnalyzeState,
                options: merge_cli,
                tree,
            })),
            CargoCli::CgPurge(purge_cli) => Ok(CgMode::Purge(CgData {
                _state: AnalyzeState,
                options: purge_cli,
                tree,
            })),
        }
    }
}

pub struct CgData<O, S> {
    _state: S,
    options: O,
    tree: ChallengeTree,
    /*
    crate_dir: PathBuf,
    crate_name: String,
    local_modules: BTreeMap<String, PathBuf>,
    my_lib: Option<PathBuf>,
    lib_modules: BTreeMap<String, PathBuf>,
    tmp_dir: PathBuf,
    tmp_input_file: PathBuf,
    tmp_output_file: PathBuf,
    output_file: PathBuf,
    line_end_chars: String,
    */
}

/*

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::{io, io::Write};
use toml::Value;
use uuid::Uuid;

impl CgData {
    fn load_output(&self, output: &mut String) -> CgResult<()> {
        *output = fs::read_to_string(self.tmp_output_file.as_path())?;
        Ok(())
    }
    fn save_output(&self, output: &String) -> CgResult<()> {
        let mut file = fs::File::create(self.tmp_output_file.as_path())?;
        file.write_all(output.as_bytes())?;
        file.flush()?;
        Ok(())
    }
    pub fn cleanup_cg_data(&self) -> CgResult<String> {
        let output = if self.options.simulate {
            "".into()
        } else if self.options.output.is_none() {
            if self.options.verbose {
                println!("create output from tmp file before clean up...");
            }
            let mut output = String::new();
            self.load_output(&mut output)?;
            output
        } else {
            if self.options.verbose {
                println!("saving output to output file...");
            }
            fs::copy(&self.tmp_output_file, &self.output_file)?;
            "".into()
        };
        if self.options.verbose {
            println!("removing tmp dir...");
        }
        // delete working tmp dir
        fs::remove_dir_all(self.tmp_dir.as_path())?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    fn test_generating_output() {
        // Act 1 - generate full output
        // set parameters
        let input = PathBuf::from(r"../csf_cg_binary_test/src/main.rs");
        let options = FusionCli {
            input: input,
            output: None,
            challenge_only: false,
            modules: "all".to_string(),
            block_hidden: "my_compass;my_array".to_string(),
            lib: "csf_cg_lib_test".to_string(),
            verbose: true,
            simulate: false,
            del_comments: false,
            keep_empty_lines: false,
        };
        // prepare output
        let mut data = CgData::new(options);
        data.prepare_cg_data().unwrap();
        data.create_output().unwrap();
        data.filter_unused_code().unwrap();

        // Act 1 - assert file content
        let output = fs::read_to_string(&data.tmp_output_file).unwrap();
        let expected_output =
            PathBuf::from(r"./test/expected_test_results/lib_tests_with_comments.rs");
        let expected_output = fs::read_to_string(expected_output).unwrap();
        assert_eq!(output, expected_output);

        // Act 2 - generate output with challenge only
        // modify options
        data.options.challenge_only = true;

        // replace current bin file with prepared test file
        let modified_file_path =
            PathBuf::from(r"./test/bin_modifications/modifications_in_challenge.rs");
        fs::copy(modified_file_path, &data.tmp_output_file).unwrap();

        // recreate output
        data.create_output().unwrap();
        data.filter_unused_code().unwrap();

        // Act 2 - assert file content
        let output = fs::read_to_string(&data.tmp_output_file).unwrap();
        assert_eq!(output, expected_output);

        // Act 3 - generate output with changes at module
        // modify options
        data.options.challenge_only = false;
        data.options.modules = "my_map_two_dim".to_string();

        // replace current bin file with prepared test file
        let modified_file_path =
            PathBuf::from(r"./test/bin_modifications/modifications_in_my_map_two_dim.rs");
        fs::copy(modified_file_path, &data.tmp_output_file).unwrap();

        // recreate output
        data.create_output().unwrap();
        data.filter_unused_code().unwrap();

        // Act 3 - assert file content
        let output = fs::read_to_string(&data.tmp_output_file).unwrap();
        assert_eq!(output, expected_output);

        // clean up tmp_file
        data.cleanup_cg_data().unwrap();
        // assert tmp file is removed
        assert!(!data.tmp_output_file.is_file());
    }

    #[test]
    fn test_generating_output_no_comments() {
        // set parameters
        let input = PathBuf::from(r"../csf_cg_binary_test/src/main.rs");
        let options = FusionCli {
            input: input,
            output: None,
            challenge_only: false,
            modules: "all".to_string(),
            block_hidden: "my_compass;my_array".to_string(),
            lib: "csf_cg_lib_test".to_string(),
            verbose: true,
            simulate: false,
            del_comments: true,
            keep_empty_lines: false,
        };

        // prepare output
        let mut data = CgData::new(options);
        data.prepare_cg_data().unwrap();
        data.create_output().unwrap();
        data.filter_unused_code().unwrap();

        // assert file content
        let output = fs::read_to_string(&data.tmp_output_file).unwrap();
        let expected_output =
            PathBuf::from(r"./test/expected_test_results/lib_test_no_comments.rs");
        let expected_output = fs::read_to_string(expected_output).unwrap();
        assert_eq!(output, expected_output);

        // clean up tmp_file
        data.cleanup_cg_data().unwrap();
        // assert tmp file is removed
        assert!(!data.tmp_output_file.is_file());
    }
    #[test]
    fn test_ult_tictactoe() {
        // set parameters
        let input = PathBuf::from(r"../../cg_ultimate_tic_tac_toe/src/main.rs");
        let output = PathBuf::from(r"../../cg_ultimate_tic_tac_toe/src/bin/codingame.rs");
        let options = FusionCli {
            input: input,
            output: Some(output),
            challenge_only: false,
            modules: "all".to_string(),
            //block_hidden: "my_array;my_line;my_rectangle".to_string(),
            block_hidden: "".to_string(),
            lib: "my_lib".to_string(),
            verbose: true,
            simulate: false,
            del_comments: false,
            keep_empty_lines: true,
        };

        // prepare output
        let mut data = CgData::new(options);
        data.prepare_cg_data().unwrap();
        data.create_output().unwrap();

        if !data.options.simulate {
            let command = if cfg!(target_os = "windows") {
                "code.cmd"
            } else {
                "code"
            };
            // open tmp_dir in VC
            Command::new(command)
                .arg(".")
                .current_dir(data.tmp_dir.as_path())
                .spawn()
                .unwrap();
        }

        data.filter_unused_code().unwrap();
        /*
        let cargo_check = data.command_cargo_check().unwrap();
        let msg_buffer: Vec<cargo_metadata::CompilerMessage> =
            cargo_metadata::Message::parse_stream(&cargo_check.stdout[..])
                .filter_map(|ps| match ps {
                    Ok(cargo_metadata::Message::CompilerMessage(msg)) => Some(msg.to_owned()),
                    _ => None,
                })
                .filter(|m| match m.message.level {
                    cargo_metadata::diagnostic::DiagnosticLevel::Error | cargo_metadata::diagnostic::DiagnosticLevel::Warning => true,
                    _ => false,
                })
                .filter(|m| !m.message.spans.is_empty())
                .collect();
        let mut msg_debug = String::new();
        use std::fmt::Write;
        write!(&mut msg_debug, "{:?}", &msg_buffer).unwrap();
        msg_debug = msg_debug.replace("\\r\\n", "\r\n\r\n");
        msg_debug = msg_debug.replace("\\n", "\n");
        std::fs::write("../output.txt", msg_debug).unwrap();
        dbg!(&data.lib_modules);
        */
        // clean up tmp_file
        if data.options.simulate {
            data.cleanup_cg_data().unwrap();
            // assert tmp file is removed
            assert!(!data.tmp_output_file.is_file());
        }
    }
}
*/

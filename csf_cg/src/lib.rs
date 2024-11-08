// central library

pub mod configuration;
pub mod preparation;
pub mod analysis;
pub mod error;
pub mod file_generation;
pub mod solve_cargo_check;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::{io, io::Write};
use toml::Value;
use uuid::Uuid;

use configuration::*;
use error::{CGError, CGResult};


pub struct CGData<S> {
    state_data: S,
    options: Cli,
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
}

/*
impl CGData {
    fn load_output(&self, output: &mut String) -> CGResult<()> {
        *output = fs::read_to_string(self.tmp_output_file.as_path())?;
        Ok(())
    }
    fn save_output(&self, output: &String) -> CGResult<()> {
        let mut file = fs::File::create(self.tmp_output_file.as_path())?;
        file.write_all(output.as_bytes())?;
        file.flush()?;
        Ok(())
    }
    pub fn cleanup_cg_data(&self) -> CGResult<String> {
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
        let options = Cli {
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
        let mut data = CGData::new(options);
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
        let options = Cli {
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
        let mut data = CGData::new(options);
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
        let options = Cli {
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
        let mut data = CGData::new(options);
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
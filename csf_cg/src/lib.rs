pub mod configuration;
pub mod file_generation;
pub mod post_generation;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::{io, io::Write};
use toml::Value;
use uuid::Uuid;

use crate::configuration::*;

/// Recursively copies the contents of one directory to another destination.
fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    // Create the destination directory if it does not exist
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    // Iterate through the source directory
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let mut dst_path = PathBuf::from(dst);
        dst_path.push(entry.file_name());

        // If it's a directory, call the function recursively
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            // Copy the file
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

pub struct CGData {
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

impl CGData {
    pub fn new(options: Cli) -> Self {
        let mut result = CGData {
            options,
            crate_dir: PathBuf::new(),
            crate_name: "".to_string(),
            local_modules: BTreeMap::new(),
            my_lib: None,
            lib_modules: BTreeMap::new(),
            tmp_dir: PathBuf::new(),
            tmp_input_file: PathBuf::new(),
            tmp_output_file: PathBuf::new(),
            output_file: PathBuf::new(),
            line_end_chars: "".to_string(),
        };
        if result.options.simulate {
            println!("Start of simulation");
            result.options.verbose = true;
        }
        if result.options.verbose {
            println!("{}", result.options);
        }
        result
    }
    pub fn prepare_cg_data(&mut self) -> BoxResult<()> {
        if self.options.verbose {
            println!("reading path of lib from toml file...");
        }
        // only accept existing main.rs as input
        if !self.options.input.is_file() || self.options.input.file_name().unwrap() != "main.rs" {
            return Err(Box::new(CGError::MustProvideInPutFile));
        }
        let crate_dir = self.options.input.as_path().parent().unwrap();
        self.crate_dir = match crate_dir.file_name().unwrap().to_str().unwrap() {
            "bin" => crate_dir.parent().unwrap().parent().unwrap().to_path_buf(),
            "src" => crate_dir.parent().unwrap().to_path_buf(),
            _ => {
                return Err(Box::new(CGError::PackageStructureError(
                    self.options.input.clone(),
                )))
            }
        };
        // get toml content
        let toml_path = self.crate_dir.join("Cargo.toml");
        if self.options.verbose {
            println!("crate_dir: {}", self.crate_dir.display());
            println!("toml_path: {}", toml_path.display());
        }
        let toml = fs::read_to_string(toml_path.clone())?.parse::<Value>()?;
        // get package name
        let package = toml
            .as_table()
            .unwrap()
            .get("package")
            .unwrap()
            .as_table()
            .unwrap();
        match package.get("name") {
            Some(crate_name) => {
                self.crate_name = crate_name.to_string().trim().replace('\"', "");
                if self.options.verbose {
                    println!("crate name: {}", self.crate_name);
                }
            }
            None => panic!("could not find package name in {}", toml_path.display()),
        }
        // get lib path, if any is used
        let dependencies = toml
            .as_table()
            .unwrap()
            .get("dependencies")
            .unwrap()
            .as_table()
            .unwrap();
        match dependencies.get(self.options.lib.as_str()) {
            Some(my_lib) => {
                let mut my_lib_path = self.crate_dir.clone();
                for lib_path_element in Path::new(
                    my_lib
                        .as_table()
                        .unwrap()
                        .get("path")
                        .unwrap()
                        .as_str()
                        .unwrap(),
                )
                .join("src")
                .iter()
                {
                    my_lib_path.push(lib_path_element);
                }
                if self.options.verbose {
                    println!(
                        "path if lib {}: {}",
                        self.options.lib,
                        my_lib_path.display()
                    );
                }
                self.my_lib = Some(my_lib_path);
            }
            None => {
                if self.options.verbose {
                    println!("lib \"{}\" not specified in toml", self.options.lib);
                }
            }
        }
        // prepare working directory
        // tmp dir must be on same path as crate dir, otherwise relative paths im Cargo.toml will not work
        self.tmp_dir = self
            .crate_dir
            .parent()
            .unwrap()
            .join(String::from(Uuid::new_v4()));
        if self.options.verbose {
            println!(
                "creating tmp working directory for cargo check: {}",
                self.tmp_dir.display()
            );
        }
        fs::create_dir_all(&self.tmp_dir)?;
        fs::copy(
            self.crate_dir.join("Cargo.toml"),
            self.tmp_dir.join("Cargo.toml"),
        )?;
        copy_dir_recursive(&self.crate_dir.join("src"), &self.tmp_dir.join("src"))?;
        if self.options.output.is_none() {
            if self.options.challenge_only || self.options.modules.as_str() != "all" {
                // these options require an already existing output file to insert changed code
                return Err(Box::new(CGError::MustProvideOutPutFile));
            }
            if self.options.verbose {
                println!("creating tmp bin file path for cargo check...");
            }
            let bin_dir = self.tmp_dir.join("src").join("bin");
            fs::create_dir_all(&bin_dir)?;
            let tmp_file = String::from(Uuid::new_v4()) + ".rs";
            self.tmp_output_file = bin_dir.join(tmp_file);
        } else {
            self.output_file = self.options.output.as_ref().unwrap().clone();
            if self.crate_dir.join("src").join("bin") != self.output_file.parent().unwrap() {
                return Err(Box::new(CGError::OutputFileError(self.output_file.clone())));
            }
            self.tmp_output_file = self
                .tmp_dir
                .join("src")
                .join("bin")
                .join(self.output_file.file_name().unwrap());
        }
        // set new variable tmp_input
        self.tmp_input_file = if self
            .options
            .input
            .as_path()
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            == "src"
        {
            self.tmp_dir.join("src").join("main.rs")
        } else {
            self.tmp_dir.join("src").join("bin").join("main.rs")
        };
        // checking for line end chars (either \n or \r\n)
        let input = fs::read_to_string(&self.tmp_input_file)?;
        self.line_end_chars = if input.contains("\r\n") {
            "\r\n".to_string()
        } else {
            "\n".to_string()
        };
        Ok(())
    }
    fn load_output(&self, output: &mut String) -> BoxResult<()> {
        *output = fs::read_to_string(self.tmp_output_file.as_path())?;
        Ok(())
    }
    fn save_output(&self, output: &String) -> BoxResult<()> {
        let mut file = fs::File::create(self.tmp_output_file.as_path())?;
        file.write_all(output.as_bytes())?;
        file.flush()?;
        Ok(())
    }
    pub fn cleanup_cg_data(&self) -> BoxResult<String> {
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
        let input = PathBuf::from(r"..\csf_cg_binary_test\src\main.rs");
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
            PathBuf::from(r".\test\expected_test_results\lib_tests_with_comments.rs");
        let expected_output = fs::read_to_string(expected_output).unwrap();
        assert_eq!(output, expected_output);

        // Act 2 - generate output with challenge only
        // modify options
        data.options.challenge_only = true;

        // replace current bin file with prepared test file
        let modified_file_path =
            PathBuf::from(r".\test\bin_modifications\modifications_in_challenge.rs");
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
            PathBuf::from(r".\test\bin_modifications\modifications_in_my_map_two_dim.rs");
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
        let input = PathBuf::from(r"..\csf_cg_binary_test\src\main.rs");
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
            PathBuf::from(r".\test\expected_test_results\lib_test_no_comments.rs");
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
        let input = PathBuf::from(r"..\..\cg_ultimate_tic_tac_toe\src\main.rs");
        let output = PathBuf::from(r"..\..\cg_ultimate_tic_tac_toe\src\bin\codingame.rs");
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
            // open tmp_dir in VC
            Command::new("code.cmd")
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

pub mod configuration;
pub mod file_generation;
pub mod post_generation;

use std::path::PathBuf;
use std::path::Path;
use std::fs;
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
    local_modules: Vec<PathBuf>,
    my_lib: PathBuf,
    lib_modules: Vec<PathBuf>,
    tmp_dir: PathBuf,
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
            local_modules: Vec::new(),
            my_lib: PathBuf::new(),
            lib_modules: Vec::new(),
            tmp_dir: PathBuf::new(),
            tmp_output_file: PathBuf::new(),
            output_file: PathBuf::new(),
            line_end_chars: "".to_string(),
        };
        if result.options.simulate {
            eprintln!("Start of simulation");
            result.options.verbose = true;
        }
        if result.options.verbose {
            eprintln!("{}", result.options);
        }
        result
    }
    pub fn prepare_cg_data(&mut self) -> BoxResult<()> {
        if self.options.verbose {
            eprintln!("reading path of lib from toml file...");
        }
        let crate_dir = self.options.input.as_path().parent().unwrap();
        self.crate_dir = match crate_dir.file_name().unwrap().to_str().unwrap() {
            "bin" => crate_dir.parent().unwrap().parent().unwrap().to_path_buf(),
            "src" => crate_dir.parent().unwrap().to_path_buf(),
            _ => return Err(Box::new(CGError::PackageStructureError(self.options.input.clone()))),
        };
        let toml_path = self.crate_dir.join("Cargo.toml");
        if self.options.verbose {
            eprintln!("crate_dir: {:?}", self.crate_dir);
            eprintln!("toml_path: {:?}", toml_path);
        }
        let toml = fs::read_to_string(toml_path.clone())?;
        let value = toml.parse::<Value>()?;
        let package = value.as_table().unwrap().get("package").unwrap().as_table().unwrap();
        match package.get("name") {
            Some(crate_name) => {
                self.crate_name = crate_name.to_string().trim().replace("\"", "");
                if self.options.verbose {
                    eprintln!("crate name: {}", self.crate_name);
                }
            }
            None => panic!("could not find package name in {:?}", toml_path),
        }
        let dependencies = value.as_table().unwrap().get("dependencies").unwrap().as_table().unwrap();
        match dependencies.get(self.options.lib.as_str()) {
            Some(my_lib) => {
                self.my_lib = toml_path;
                self.my_lib.pop();
                for lib_path_element in Path::new(my_lib.as_table().unwrap().get("path").unwrap().as_str().unwrap()).join("src").iter() {
                    self.my_lib.push(lib_path_element);
                }
                if self.options.verbose {
                    eprintln!("path if lib {}: {:?}", self.options.lib, self.my_lib);
                }
            },
            None => {
                if self.options.verbose {
                    eprintln!("lib \"{}\" not specified in toml", self.options.lib);
                }
            },
        }
        // prepare working directory
        // tmp dir must be on same path as crate dir, otherwise relative paths im Cargo.toml will not work
        self.tmp_dir = self.crate_dir.parent().unwrap().join(String::from(Uuid::new_v4()));
        if self.options.verbose {
            eprintln!("creating tmp working directory for cargo check: {:#?}", self.tmp_dir.as_path());
        }
        fs::create_dir_all(&self.tmp_dir)?;
        fs::copy(self.crate_dir.join("Cargo.toml"), self.tmp_dir.join("Cargo.toml"))?;
        copy_dir_recursive(&self.crate_dir.join("src"), &self.tmp_dir.join("src"))?;
        if self.options.output.is_none() {
            if self.options.challenge_only || self.options.modules.as_str() != "all" {
                // these options require an already existing output file to insert changed code
                return Err(Box::new(CGError::MustProvideOutPutFile));
            }
            if self.options.verbose {
                eprintln!("creating tmp file path for cargo check...");
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
            self.tmp_output_file = self.tmp_dir.join("src").join("bin").join(self.output_file.file_name().unwrap());
        }
        // checking for line end chars (either \n or \r\n)
        let input = fs::read_to_string(self.options.input.as_path())?;
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
                eprintln!("create output from tmp file before clean up...");
            }
            let mut output = String::new();
            self.load_output(&mut output)?;
            output
        } else {
            if self.options.verbose {
                eprintln!("saving output to output file...");
            }
            fs::copy(&self.tmp_output_file, &self.output_file)?;
            "".into()
        };
        if self.options.verbose {
            eprintln!("removing tmp dir...");
        }
        // delete working tmp dir
        fs::remove_dir_all(self.tmp_dir.as_path())?;
        Ok(output)
    } 
}

#[cfg(test)]
mod tests {
    
    use super::*;
    use std::path::PathBuf;
    use std::{fs, fs::File};
    use std::io::Write;

    #[test]
    fn test_output_with_blocked_modules() {
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
        };
        // prepare output
        let mut data = CGData::new(options);
        data.prepare_cg_data().unwrap();
        data.create_output().unwrap();
        data.filter_unused_code().unwrap();
        
        // assert file content
        let output= fs::read_to_string(&data.tmp_output_file).unwrap();
        let expected_output = PathBuf::from(r".\test\expected_test_results\lib_tests_with_comments.rs");
        let expected_output= fs::read_to_string(expected_output).unwrap();
        assert_eq!(output, expected_output);

        // clean up tmp_file
        data.cleanup_cg_data().unwrap();
        // assert tmp file is removed
        assert!(!data.tmp_output_file.is_file());
    }

    #[test]
    fn test_output_file_relative_path_block_module_challenge_only() {
        let input = PathBuf::from(r"..\csf_cg_binary_test\src\main.rs");
        let output = PathBuf::from(r"..\csf_cg_binary_test\src\bin\codingame.rs");
        let options = Cli {
            input: input,
            output: Some(output.clone()),
            challenge_only: true,
            modules: "all".to_string(),
            block_hidden: "".to_string(),
            lib: "csf_cg_lib_test".to_string(),
            verbose: true,
            simulate: false,
            del_comments: false,
        };
        // first modify main section by overwriting file content
        let modified_file_path = PathBuf::from(r".\test\codingame_with_modifications.rs");
        let modified_file_content= fs::read_to_string(modified_file_path).unwrap();
        let mut modified_file = File::create(output.clone()).unwrap();
        modified_file.write_all(modified_file_content.as_bytes()).unwrap();
        // now let's run
        let mut data = CGData::new(options);
        data.prepare_cg_data().unwrap();
        data.create_output().unwrap();
        data.filter_unused_code().unwrap();
        
        let output= fs::read_to_string(output).unwrap();
        let expected_output = PathBuf::from(r".\test\expected_test_results\lib_tests_with_comments.rs");
        let expected_output= fs::read_to_string(expected_output).unwrap();
        assert_eq!(output, expected_output);
    }

    #[test]
    fn test_output_file_relative_path_block_module_my_map_two_dim() {
        let input = PathBuf::from(r"..\csf_cg_binary_test\src\main.rs");
        let output = PathBuf::from(r"..\csf_cg_binary_test\src\bin\codingame.rs");
        let options = Cli {
            input: input,
            output: Some(output.clone()),
            challenge_only: false,
            modules: "my_map_two_dim".to_string(),
            block_hidden: "my_compass;my_array".to_string(),
            lib: "csf_cg_lib_test".to_string(),
            verbose: true,
            simulate: false,
            del_comments: false,
        };
        // first modify main section by overwriting file content
        let modified_file_path = PathBuf::from(r".\test\codingame_with_modifications_in_my_map_two_dim.rs");
        let modified_file_content= fs::read_to_string(modified_file_path).unwrap();
        let mut modified_file = File::create(output.clone()).unwrap();
        modified_file.write_all(modified_file_content.as_bytes()).unwrap();
        // now let's run
        let mut data = CGData::new(options);
        data.prepare_cg_data().unwrap();
        data.create_output().unwrap();
        data.filter_unused_code().unwrap();
        
        let output= fs::read_to_string(output).unwrap();
        let expected_output = PathBuf::from(r".\test\expected_test_results\lib_tests_with_comments.rs");
        let expected_output= fs::read_to_string(expected_output).unwrap();
        assert_eq!(output, expected_output);
    }

    #[test]
    fn test_output_stdout_relative_path_block_module() {
        let input = PathBuf::from(r"..\csf_cg_binary_test\src\main.rs");
        let output = None;
        let options = Cli {
            input: input,
            output,
            challenge_only: false,
            modules: "all".to_string(),
            block_hidden: "my_compass;my_array".to_string(),
            lib: "csf_cg_lib_test".to_string(),
            verbose: true,
            simulate: false,
            del_comments: false,
        };
        let mut data = CGData::new(options);
        data.prepare_cg_data().unwrap();
        data.create_output().unwrap();
        data.filter_unused_code().unwrap();
        
        let output= fs::read_to_string(data.tmp_output_file.as_path()).unwrap();
        let expected_output = PathBuf::from(r".\test\expected_test_results\lib_tests_with_comments.rs");
        let expected_output= fs::read_to_string(expected_output).unwrap();
        assert_eq!(output, expected_output);
        data.cleanup_cg_data().unwrap();
    }

    #[test]
    fn test_output_file_relative_path_block_module_no_comments() {
        let input = PathBuf::from(r"..\csf_cg_binary_test\src\main.rs");
        let output = PathBuf::from(r"..\csf_cg_binary_test\src\bin\codingame_no_comments.rs");
        let options = Cli {
            input: input,
            output: Some(output.clone()),
            challenge_only: false,
            modules: "all".to_string(),
            block_hidden: "my_compass;my_array".to_string(),
            lib: "csf_cg_lib_test".to_string(),
            verbose: true,
            simulate: false,
            del_comments: true,
        };
        let mut data = CGData::new(options);
        data.prepare_cg_data().unwrap();
        data.create_output().unwrap();
        data.filter_unused_code().unwrap();
        
        let output= fs::read_to_string(output).unwrap();
        let expected_output = PathBuf::from(r".\test\expected_test_results\lib_tests_with_comments_no_comments.rs");
        let expected_output= fs::read_to_string(expected_output).unwrap();
        assert_eq!(output, expected_output);
    }
}
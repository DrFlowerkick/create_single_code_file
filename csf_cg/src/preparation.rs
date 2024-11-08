// preparation of working environment

// state for preparation
pub struct PrepState;

/*
use super::{CGData, configuration::OutputMode, analysis::AnaState, error::{CGResult, CGError}};
use std::path::{Path, PathBuf};
use std::{fs, io};
use anyhow::Context;
use uuid::Uuid;
use toml::Value;

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

/// get relative path from crate dir to input file
fn relative_path_of_input(input: &Path) -> PathBuf {
    let mut rel_input_file_path = PathBuf::new();
    let mut input_stem = input.to_path_buf();
    loop {
        let file_name = input_stem.file_name().unwrap().to_owned();
        input_stem = input_stem.parent().unwrap().to_path_buf();
        rel_input_file_path = Path::new(&file_name).join(rel_input_file_path);
        if file_name == "src" {
            rel_input_file_path = Path::new(".").join(rel_input_file_path);
            break;
        }
    }
    rel_input_file_path
}

/// Implement CGData for PrepState

impl CGData<PrepState> {
    pub fn prepare_working_environment(&mut self) -> CGResult<CGData<AnaState>> {
        if self.options.verbose {
            println!("reading path of lib from toml file...");
        }
        // input file must exist and input must be .rs file
        if !self.options.input.is_file() || match self.options.input.extension() {
            Some(extension) => extension != "rs",
            None => true,
        } {
            return Err(CGError::MustProvideValidInputFilePath(self.options.input.clone()));
        }
        let input_dir_name = self.options.input.parent().unwrap();
        let crate_dir = match input_dir_name.file_name().unwrap().to_str().unwrap() {
            "bin" => {
                // bin crate in ./src/bin must have other name then options.filename
                let mut input_file = self.options.input.clone();
                // remove extensions
                while input_file.set_extension("") {}
                let input_file_name = input_file.file_name().unwrap().to_str().unwrap();
                if input_file_name == self.options.filename {
                    return Err(CGError::MustProvideValidOutputFileName(self.options.filename.clone()));
                }
                input_dir_name.parent().unwrap().parent().unwrap().to_path_buf()
            },
            "src" => {
                // bin crate in ./src must be main.rs
                let input_file_name = self.options.input.file_name().unwrap();
                if input_file_name != "main.rs" {
                    return Err(CGError::PackageStructureError(self.options.input.clone()));
                }
                input_dir_name.parent().unwrap().to_path_buf()
            },
            _ => {
                return Err(CGError::PackageStructureError(self.options.input.clone()))
            }
        };
        let rel_input_file_2 = self.options.input.strip_prefix(crate_dir);
        let rel_input_file = relative_path_of_input(self.options.input.as_path());
        // prepare relative output file path
        let output_file_name = Path::new(&self.options.filename);
        if output_file_name.extension().is_some() {
            return Err(CGError::MustProvideValidOutputFileName(self.options.filename.clone()));
        }
        let mut relative_output_path = Path::new("./src/bin").join(output_file_name);
        match self.options.output_mode {
            OutputMode::Merge | OutputMode::Update => {
                relative_output_path.set_extension("rs");
            },
            OutputMode::Increment => {

            },
        }

        // get toml content
        let toml_path = crate_dir.join("Cargo.toml");
        if self.options.verbose {
            println!("crate_dir: {}", crate_dir.display());
            println!("toml_path: {}", toml_path.display());
        }
        let toml = fs::read_to_string(toml_path.clone())
            .context("Loading toml file of input crate failed.")?
            .parse::<Value>()
            .context("Parsing toml file of input crate failed.")?;
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
                let mut my_lib_path = crate_dir.clone();
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
                    println!("lib '{}' not specified in toml", self.options.lib);
                }
            }
        }
        // prepare working directory
        // tmp dir must be on same path as crate dir, otherwise relative paths im Cargo.toml will not work
        let tmp_dir = crate_dir
            .parent()
            .unwrap()
            .join(String::from(Uuid::new_v4()));
        if self.options.verbose {
            println!(
                "creating tmp working directory for cargo check: {}",
                self.tmp_dir.display()
            );
        }
        fs::create_dir_all(&self.tmp_dir)
        .context("Creation of temporary working directory failed.")?;
        fs::copy(
            crate_dir.join("Cargo.toml"),
            tmp_dir.join("Cargo.toml"),
        )
        .context("Copying of input crate toml file to temporary working directory failed.")?;
        let bin_dir = self.tmp_dir.join("src").join("bin");
        fs::create_dir_all(&bin_dir).context("Creation of ./src/bin directory in temporary working directory failed.")?;
        copy_dir_recursive(&crate_dir.join("src"), &tmp_dir.join("src"))
        .context("Recursive copying of input crate src directory to temporary working directory failed.")?;
        if self.options.output.is_none() {
            if self.options.challenge_only || self.options.modules.as_str() != "all" {
                // these options require an already existing output file to insert changed code
                return Err(Box::new(CGError::MustProvideOutPutFile));
            }
            if self.options.verbose {
                println!("creating tmp bin file path for cargo check...");
            }
            let tmp_file = String::from(Uuid::new_v4()) + ".rs";
            self.tmp_output_file = bin_dir.join(tmp_file);
        } else {
            self.output_file = self.options.output.as_ref().unwrap().clone();
            if crate_dir.join("src").join("bin") != self.output_file.parent().unwrap() {
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
        let input = fs::read_to_string(&self.tmp_input_file)
        .context("Loading input file for checking line end chars failed.")?;
        self.line_end_chars = if input.contains("\r\n") {
            "\r\n".to_string()
        } else {
            "\n".to_string()
        };
        Ok(())
    }
}
*/
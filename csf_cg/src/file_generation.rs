use super::*;
use std::fmt;
use std::fs;
use std::path::Path;
use syn::{visit::Visit, File, ItemUse, ItemMod, UseTree};

use crate::configuration::*;

// Struct to visit source file and collect certain statements
struct SrcVisitor {
    uses: Vec<ItemUse>,
    mods: Vec<ItemMod>,
}

impl<'ast> Visit<'ast> for SrcVisitor {
    fn visit_item_use(&mut self, i: &'ast ItemUse) {
        self.uses.push(i.clone());
    }
    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        self.mods.push(i.clone());
    }
}

#[derive(PartialEq)]
enum ModuleType {
    Local,
    Lib,
    Hidden(String),
}

impl ModuleType {
    fn is_hidden(&self) -> bool {
        matches!(self, ModuleType::Hidden(_))
    }
    fn hidden_source(&self) -> &String {
        match self {
            ModuleType::Hidden(source) => source,
            _ => panic!("self is not hidden"),
        }
    }
}

impl CGData {
    fn get_modules_from_use_line<'a>(
        &mut self,
        mod_type: ModuleType,
        module_path_iter: impl Iterator<Item = &'a str>,
    ) {
        let mut start_path = match mod_type {
            ModuleType::Local => self.crate_dir.join("src"),
            _ => self.my_lib.clone(),
        };
        let module_list = match mod_type {
            ModuleType::Local => &mut self.local_modules,
            _ => &mut self.lib_modules,
        };
        for module in module_path_iter {
            if self.options.modules.as_str() == "all" || self.options.modules.split(';').any(|m| m == module) {
                let mut path = start_path.join(module);
                path.set_extension("rs");
                if mod_type.is_hidden() && self.options.block_hidden.split(';').any(|m| m == module) {
                    if self.options.verbose {
                        println!(
                            "blocked hidden module {} (found in {:?})...",
                            module,
                            mod_type.hidden_source()
                        );
                    }
                    break;
                } else if path.is_file() {
                    // found locale module
                    if !module_list.iter().any(|p| *p == path) {
                        // add locale module to list
                        if self.options.verbose {
                            match mod_type {
                                ModuleType::Local => println!(
                                    "found locale module \"{}\", adding {} to module list...",
                                    module, path.display()
                                ),
                                ModuleType::Lib => println!(
                                    "found lib module \"{}\", adding {} to module list...",
                                    module, path.display()
                                ),
                                ModuleType::Hidden(ref source) => println!(
                                    "found hidden module {} in {}, adding {} to module list...",
                                    module, source, path.display()
                                ),
                            }
                        }
                        module_list.push(path);
                    }
                    // module dir, if sub module(s) are in path
                    start_path = start_path.join(module);
                }
            } else {
                break;
            }
        }
    }
    fn get_local_modules_v2(&mut self) -> BoxResult<()> {
        // get local modules if either modules is set to all or contains the keyword lib
        if !(self.options.modules.as_str() == "all"
            || self.options.modules.split(';').any(|m| m == "lib"))
        {
            if self.options.verbose {
                println!("\"lib\" (or \"all\") not in given list of modules -> skipping collecting path of local modules of crate...");
            }
            return Ok(());
        }
        if self.options.verbose {
            println!("collecting path of all local modules of crate...");
        }
        // initialize parse_path; if lib.rs is used in main.rs, it will be added to parse_path
        // add all module path to parse_path until no more module is found
        let mut parse_path: Vec<PathBuf> = vec![self.tmp_input_file.clone()];
        let mut index = 0;
        while index < parse_path.len() {
            // load source code
            let code = fs::read_to_string(&parse_path[index])?;
            // Parse the source code into a syntax tree
            let syntax: File = syn::parse_file(&code)?;
            // Create a visitor to find use statements
            let mut visitor = SrcVisitor { uses: Vec::new(), mods: Vec::new() };
            // Visit the syntax tree and collect all use statements
            visitor.visit_file(&syntax);
            // parse main.rs input file for crate lib.rs (use project_name::*;)
            if parse_path[index].file_name().unwrap().to_str().unwrap() == "main.rs" {
                if visitor.uses.iter().any(|v| match &v.tree {
                    UseTree::Path(use_path) => use_path.ident.to_string() == self.crate_name,
                    _ => false
                }) {
                    let path = self.crate_dir.join("src").join("lib.rs");
                    if self.options.verbose {
                        println!(
                            "found library crate, adding {} to module list...",
                            path.display()
                        );
                    }
                    self.local_modules.push(path.clone());
                    parse_path.push(path);
                }
            }
            // set directory, which contains the modules, if there are some
            let mod_dir = match parse_path[index].file_name().unwrap().to_str().unwrap() {
                "main.rs" | "lib.rs" | "mod.rs" => parse_path[index].parent().unwrap().to_path_buf(),
                _ => {
                    let mut mod_path = parse_path[index].clone();
                    mod_path.set_extension("");
                    mod_path
                },
            };
            
            // increment index before checking, if mod_dir exists
            index += 1;
            // if mod_dir does not exist, continue while loop, since module cannot contain further modules.
            if !mod_dir.is_dir() {
                continue;
            }
            // parse mod entries, which are empty
            for item_mod in visitor.mods.iter().filter(|m| m.content.is_none()) {
                let mut path = mod_dir.join(item_mod.ident.to_string() + ".rs");
                if !path.is_file() {
                    path.set_extension("");
                    path = path.join("mod.rs");
                    assert!(path.is_file());
                }
                if self.options.verbose {
                    println!(
                        "found library crate, adding {} to module list...",
                        path.display()
                    );
                }
                self.local_modules.push(path.clone());
                parse_path.push(path);
            }
        }
        

        Ok(())
    }
    fn get_local_modules(&mut self) -> BoxResult<()> {
        // get local modules if either modules is set to all or contains the keyword lib
        if !(self.options.modules.as_str() == "all"
            || self.options.modules.split(';').any(|m| m == "lib"))
        {
            if self.options.verbose {
                println!("\"lib\" (or \"all\") not in given list of modules -> skipping collecting path of local modules of crate...");
            }
            return Ok(());
        }
        if self.options.verbose {
            println!("collecting path of all local modules of crate...");
        }
        let mut input = String::new();
        self.load(self.tmp_input_file.as_path(), &mut input)?;
        // search for usage of lib.rs, which is referenced by "use <crate name>::*;"
        let lib_pattern = "use ".to_string() + self.crate_name.as_str() + "::";
        for line in input
            .lines()
            .map(|l| l.trim())
            .filter(|l| l.starts_with(lib_pattern.as_str()))
        {
            let path = self.crate_dir.join("src").join("lib.rs");
            if !self.local_modules.iter().any(|p| *p == path) {
                if self.options.verbose {
                    println!(
                        "found library crate, adding {} to module list...",
                        path.display()
                    );
                }
                self.local_modules.push(path);
            }
            let module_path_iter = line
                .split(&[':', ';'][..])
                .filter(|m| !m.is_empty())
                .skip(1);
            self.get_modules_from_use_line(ModuleType::Local, module_path_iter);
        }
        // search for further local modules in lib.rs (and possibly other already referenced local modules)
        let mut index = 0;
        while index < self.local_modules.len() {
            let mut input = String::new();
            self.load(self.local_modules[index].as_path(), &mut input)?;
            for line in input
                .lines()
                .map(|l| l.trim())
                .filter(|l| l.starts_with("use crate::"))
            {
                let module_path_iter = line
                    .split(&[':', ';'][..])
                    .filter(|m| !m.is_empty())
                    .skip(1);
                self.get_modules_from_use_line(ModuleType::Local, module_path_iter);
            }
            index += 1;
        }
        Ok(())
    }
    fn get_lib_modules(&mut self) -> BoxResult<()> {
        if self.options.challenge_only {
            if self.options.verbose {
                println!(
                    "challenge_only -> skipping collecting path of all specified modules of lib..."
                );
            }
            return Ok(());
        }
        if !self.my_lib.is_dir() {
            if self.options.verbose {
                println!("lib \"{}\" not specified in toml -> skipping collecting path of all specified modules of lib...", self.options.lib);
            }
            return Ok(());
        }
        if self.options.verbose {
            println!("collecting path of all specified modules of lib...");
        }
        let mut source_files = self.local_modules.clone();
        source_files.push(self.tmp_input_file.clone());
        for module in source_files.iter() {
            let mut input = String::new();
            self.load(module, &mut input)?;
            let lib_pattern = "use ".to_string() + self.options.lib.as_str() + "::";
            for line in input
                .lines()
                .map(|l| l.trim())
                .filter(|l| l.starts_with(lib_pattern.as_str()))
            {
                let module_path_iter = line
                    .split(&[':', ';'][..])
                    .filter(|m| !m.is_empty())
                    .skip(1);
                self.get_modules_from_use_line(ModuleType::Lib, module_path_iter);
            }
        }
        // if all modules are required, search for hidden internal modules in local lib and add them to modules
        if self.options.modules.as_str() == "all" {
            if self.options.verbose {
                println!("collecting path of all hidden modules of lib...");
            }
            let mut index = 0;
            while index < self.lib_modules.len() {
                let mod_path = self.lib_modules[index].as_path();
                let mod_name = mod_path.file_stem().unwrap().to_str().unwrap().to_string();
                let mut input = String::new();
                self.load(mod_path, &mut input)?;
                for line in input
                    .lines()
                    .filter(|l| l.trim().starts_with("use crate::"))
                {
                    let module_path_iter = line
                        .split(&[':', ';'][..])
                        .filter(|m| !m.is_empty())
                        .skip(1);
                    self.get_modules_from_use_line(
                        ModuleType::Hidden(mod_name.clone()),
                        module_path_iter,
                    );
                }
                index += 1;
            }
        }
        Ok(())
    }
    fn load(&self, path: &Path, output: &mut String) -> BoxResult<()> {
        // read in the file defined by path
        let mut data = fs::read_to_string(path)?;
        // remove tests if existing
        if let Some(byte_index) = data.find("#[cfg(test)]") {
            data.truncate(byte_index);
        }
        data = data.replace("pub ", "");
        if !output.is_empty() {
            output.push_str(self.line_end_chars.as_str());
        }
        // append to file data to output, including markers for current file
        fmt::write(
            output,
            format_args!(
                "//⏬{}{}{}{}//⏫{}",
                path.file_name().unwrap().to_str().unwrap(),
                self.line_end_chars,
                data.trim(),
                self.line_end_chars,
                path.file_name().unwrap().to_str().unwrap()
            ),
        )?;
        Ok(())
    }
    fn load_lib(&self, path: &Path, output: &mut String) -> BoxResult<()> {
        if self.options.verbose {
            println!("loading lib module {:?}...", path.file_name().unwrap());
        }
        self.load(path, output)?;
        // filter usage of modules of crate, since all modules will be copied into one single file
        *output = output
            .lines()
            .filter(|l| !l.trim().starts_with("use crate::"))
            .collect::<Vec<&str>>()
            .join(self.line_end_chars.as_str());
        Ok(())
    }
    fn load_challenge(&self, path: &Path, output: &mut String) -> BoxResult<()> {
        if self.options.verbose {
            println!("loading challenge code {:?}...", path.file_name().unwrap());
        }
        self.load(path, output)?;
        // remove lines including use of lib, local crate or modules of local crate
        let lib_pattern = "use ".to_string() + self.options.lib.as_str() + "::";
        let local_crate_pattern = "use ".to_string() + self.crate_name.as_str() + "::";
        *output = output
            .lines()
            .filter(|l| {
                !(l.trim().starts_with(lib_pattern.as_str())
                    || l.trim().starts_with(local_crate_pattern.as_str())
                    || l.trim().starts_with("use crate::"))
            })
            .collect::<Vec<&str>>()
            .join(self.line_end_chars.as_str());
        Ok(())
    }
    fn insert(&self, input: &mut str, output: &mut String) -> BoxResult<()> {
        let start_marker = input.lines().next().unwrap().to_string() + self.line_end_chars.as_str();
        let end_marker = input.lines().last().unwrap().to_string();
        let pre_start_marker = output
            .split(start_marker.as_str())
            .next()
            .unwrap()
            .to_string();
        let post_end_marker = output.split(end_marker.as_str()).last().unwrap();
        *output = pre_start_marker + input + self.line_end_chars.as_str() + post_end_marker;
        Ok(())
    }
    fn insert_lib(&self, output: &mut String) -> BoxResult<()> {
        for path in self.lib_modules.iter() {
            let mut input = String::new();
            self.load_lib(path, &mut input)?;
            if self.options.verbose {
                println!("inserting {:?} into output...", path.file_name().unwrap());
            }
            self.insert(&mut input, output)?;
        }
        Ok(())
    }
    fn insert_challenge(&self, output: &mut String) -> BoxResult<()> {
        let mut files = self.local_modules.clone();
        files.push(self.tmp_input_file.clone());
        for file_input in files.iter() {
            let mut input = String::new();
            self.load_challenge(file_input, &mut input)?;
            if self.options.verbose {
                println!(
                    "inserting {:?} into output...",
                    self.tmp_input_file.file_name().unwrap()
                );
            }
            self.insert(&mut input, output)?;
        }
        Ok(())
    }
    pub fn create_output(&mut self) -> BoxResult<()> {
        self.get_local_modules()?;
        self.get_lib_modules()?;
        let mut output = String::new();
        if self.options.challenge_only {
            if self.options.verbose {
                println!("insert option challenge_only is active");
            }
            self.load_output(&mut output)?;
            self.insert_challenge(&mut output)?;
        } else if self.options.modules.as_str() != "all" {
            if self.options.verbose {
                println!(
                    "insert option specific module(s) is active: {}",
                    self.options.modules
                );
            }
            self.load_output(&mut output)?;
            self.insert_challenge(&mut output)?;
            self.insert_lib(&mut output)?;
        } else {
            for path in self.lib_modules.iter() {
                self.load_lib(path.as_path(), &mut output)?;
            }
            for path in self.local_modules.iter() {
                self.load_challenge(path.as_path(), &mut output)?;
            }
            self.load_challenge(self.tmp_input_file.as_path(), &mut output)?;
        }
        if self.options.simulate {
            println!("End of simulation");
        } else {
            if self.options.verbose {
                println!(
                    "saving output into tmp file {:#?}",
                    self.tmp_output_file.as_path()
                );
            }
            self.save_output(&output)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_simulation_output() {
        let input = PathBuf::from(r"..\csf_cg_binary_test\src\main.rs");
        let options = Cli {
            input: input,
            output: None,
            challenge_only: false,
            modules: "all".to_string(),
            block_hidden: "".to_string(),
            lib: "csf_cg_lib_test".to_string(),
            verbose: false,
            simulate: true,
            del_comments: false,
        };
        // simulate output
        let mut data = CGData::new(options);
        data.prepare_cg_data().unwrap();
        data.create_output().unwrap();

        // assert no file is created
        assert!(!data.tmp_output_file.is_file());

        // clean up tmp dir
        data.cleanup_cg_data().unwrap();
        // assert tmp file is removed
        assert!(!data.tmp_dir.is_dir());
    }

    #[test]
    fn test_simulation_output_with_block_hidden_modules() {
        let input = PathBuf::from(r"..\csf_cg_binary_test\src\main.rs");
        let options = Cli {
            input: input,
            output: None,
            challenge_only: false,
            modules: "all".to_string(),
            block_hidden: "my_compass;my_array".to_string(),
            lib: "csf_cg_lib_test".to_string(),
            verbose: false,
            simulate: true,
            del_comments: false,
        };
        // simulate output
        let mut data = CGData::new(options);
        data.prepare_cg_data().unwrap();
        data.create_output().unwrap();

        // assert no file is created
        assert!(!data.tmp_output_file.is_file());

        // clean up tmp dir
        data.cleanup_cg_data().unwrap();
        // assert tmp file is removed
        assert!(!data.tmp_dir.is_dir());
    }

    #[test]
    fn test_creation_tmp_file_output() {
        let input = PathBuf::from(r"..\csf_cg_binary_test\src\main.rs");
        let options = Cli {
            input: input,
            output: None,
            challenge_only: false,
            modules: "all".to_string(),
            block_hidden: "".to_string(),
            lib: "csf_cg_lib_test".to_string(),
            verbose: false,
            simulate: false,
            del_comments: false,
        };
        // create output
        let mut data = CGData::new(options);
        data.prepare_cg_data().unwrap();
        data.create_output().unwrap();

        // assert tmp file is created
        assert!(data.tmp_output_file.is_file());

        // assert file content
        let mut file_content = String::new();
        data.load_output(&mut file_content).unwrap();
        let expected_file_content = fs::read_to_string(PathBuf::from(
            r".\test\expected_test_results\test_creation_tmp_file_output.rs",
        ))
        .unwrap();
        assert_eq!(file_content, expected_file_content);

        // clean up tmp dir
        data.cleanup_cg_data().unwrap();
        // assert tmp file is removed
        assert!(!data.tmp_dir.is_dir());
    }

    #[test]
    fn test_new_local_mods_identification() {
        let input = PathBuf::from(r"..\csf_cg_binary_test\src\main.rs");
        let options = Cli {
            input: input,
            output: None,
            challenge_only: false,
            modules: "all".to_string(),
            block_hidden: "".to_string(),
            lib: "csf_cg_lib_test".to_string(),
            verbose: false,
            simulate: true,
            del_comments: false,
        };
        // create output
        let mut data = CGData::new(options);
        data.prepare_cg_data().unwrap();
        data.get_local_modules_v2().unwrap();
        
        // clean up tmp dir
        data.cleanup_cg_data().unwrap();
        // assert tmp file is removed
        assert!(!data.tmp_dir.is_dir());
    }
}

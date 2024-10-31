use super::*;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;
use syn::{visit::Visit, File, ItemMod, ItemUse, UseTree};
use toml::Value;

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

impl SrcVisitor {
    fn new(path: &PathBuf) -> BoxResult<SrcVisitor> {
        // load source code
        let code = fs::read_to_string(path)?;
        // Parse the source code into a syntax tree
        let syntax: File = syn::parse_file(&code)?;
        // Create a visitor to find use statements
        let mut visitor = SrcVisitor {
            uses: Vec::new(),
            mods: Vec::new(),
        };
        // Visit the syntax tree and collect all use statements
        visitor.visit_file(&syntax);
        Ok(visitor)
    }
}

#[derive(Clone)]
enum ParseUseMode {
    InputCrate,
    // String is the name of the module
    // Vec<String> is list of my_lib dependencies (incl. std), which must be ignored
    HiddenModulesInMyLib((String, Vec<String>)),
}

impl CGData {
    fn parse_mod_from_src_file(
        &self,
        src_path: PathBuf,
        current_module: String,
        modules: &mut BTreeMap<String, PathBuf>,
        verbose: bool,
    ) -> BoxResult<()> {
        // set directory, which contains the modules, if there are some
        let mod_dir = match src_path.file_name().unwrap().to_str().unwrap() {
            "main.rs" | "lib.rs" | "mod.rs" => src_path.parent().unwrap().to_path_buf(),
            _ => {
                let mut mod_dir = src_path.clone();
                mod_dir.set_extension("");
                mod_dir
            }
        };

        // if mod_dir does not exist, it cannot contain further modules.
        // therefore no parsing required
        if !mod_dir.is_dir() {
            return Ok(());
        }

        // create visitor from source code
        let visitor = SrcVisitor::new(&src_path)?;

        // parse mod entries, which are empty
        for item_mod in visitor.mods.iter().filter(|m| m.content.is_none()) {
            let mut module = item_mod.ident.to_string();
            let mut path = mod_dir.join(module.clone() + ".rs");
            if !path.is_file() {
                path.set_extension("");
                path = path.join("mod.rs");
                assert!(path.is_file());
            }
            module = current_module.clone() + "::" + &module;
            if modules.insert(module.clone(), path.clone()).is_some() {
                // module already in collection
                continue;
            }
            if self.options.verbose && verbose {
                println!(
                    "found module \"{}\", adding {} to module list...",
                    module,
                    path.display()
                );
            }
            self.parse_mod_from_src_file(path, module, modules, verbose)?;
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
        // init local modules collection
        let mut local_modules = BTreeMap::new();
        // parse modules of bin_crate
        if self.options.verbose {
            println!("collecting \"{}\" modules of bin_crate...", self.crate_name);
        }
        self.parse_mod_from_src_file(
            self.tmp_input_file.clone(),
            "bin_crate".into(),
            &mut local_modules,
            true,
        )?;

        // parse main.rs input file for crate lib.rs (use project_name::*;)
        // create visitor from source code
        let visitor = SrcVisitor::new(&self.tmp_input_file)?;
        if visitor.uses.iter().any(|v| match &v.tree {
            UseTree::Path(use_path) => use_path.ident == self.crate_name,
            _ => false,
        }) {
            // set path to lib.rs
            let lib_rs = self.crate_dir.join("src").join("lib.rs");
            // add lib.rs to local_modules
            if self.options.verbose {
                println!(
                    "found module \"{}\", adding {} to module list...",
                    self.crate_name,
                    lib_rs.display(),
                );
            }
            local_modules.insert(self.crate_name.clone(), lib_rs.clone());
            // parse modules of lib_crate
            if self.options.verbose {
                println!("collecting \"{}\" modules of lib_crate...", self.crate_name);
            }
            self.parse_mod_from_src_file(lib_rs, "lib_crate".into(), &mut local_modules, true)?;
        }
        // set local_modules
        self.local_modules = local_modules;
        Ok(())
    }
    fn parse_use_item(
        &mut self,
        use_tree: &UseTree,
        mod_name: String,
        parse_mode: &ParseUseMode,
        lib_modules: &BTreeMap<String, PathBuf>,
    ) {
        match use_tree {
            UseTree::Path(use_path) => {
                let module = use_path.ident.to_string();
                match parse_mode {
                    ParseUseMode::InputCrate => {
                        if mod_name.is_empty() {
                            if module != self.options.lib {
                                // use statement does not refer to my_lib
                                return;
                            }
                            self.parse_use_item(&use_path.tree, module, parse_mode, lib_modules);
                        } else {
                            let extend_mod_name = mod_name + "::" + &module;
                            self.parse_use_item(
                                &use_path.tree,
                                extend_mod_name,
                                parse_mode,
                                lib_modules,
                            );
                        }
                    }
                    ParseUseMode::HiddenModulesInMyLib((src_module, dependencies)) => {
                        match module.as_str() {
                            "crate" => self.parse_use_item(
                                &use_path.tree,
                                self.options.lib.clone(),
                                parse_mode,
                                lib_modules,
                            ),
                            "self" => self.parse_use_item(
                                &use_path.tree,
                                mod_name,
                                parse_mode,
                                lib_modules,
                            ),
                            "super" => {
                                if mod_name == self.options.lib {
                                    panic!("\"use super::\" in crate module should not happen.");
                                }
                                let mut super_mod_name = mod_name.clone();
                                while let Some(c) = super_mod_name.pop() {
                                    if c == ':' {
                                        super_mod_name.pop();
                                        break;
                                    }
                                }
                                self.parse_use_item(
                                    &use_path.tree,
                                    super_mod_name,
                                    parse_mode,
                                    lib_modules,
                                );
                            }
                            _ => {
                                if dependencies.contains(&module) {
                                    // ignore my_lib dependencies
                                    return;
                                }
                                if self.options.block_hidden.split(';').any(|b| b == module) {
                                    // block hidden module
                                    if self.options.verbose {
                                        println!(
                                            "blocked hidden module {} (found in {})...",
                                            module, src_module
                                        );
                                    }
                                    return;
                                }
                                let extend_mod_name = mod_name + "::" + &module;
                                self.parse_use_item(
                                    &use_path.tree,
                                    extend_mod_name,
                                    parse_mode,
                                    lib_modules,
                                );
                            }
                        }
                    }
                }
            }
            UseTree::Group(use_group) => {
                for group_item in use_group.items.iter() {
                    self.parse_use_item(group_item, mod_name.clone(), parse_mode, lib_modules);
                }
            }
            UseTree::Glob(_) | UseTree::Name(_) | UseTree::Rename(_) => {
                // add mod_name to use_statements
                if self.lib_modules.contains_key(&mod_name) {
                    // already added to lib_modules
                    return;
                }
                let path = lib_modules.get(&mod_name).unwrap();
                if self.options.verbose {
                    match parse_mode {
                        ParseUseMode::InputCrate => println!(
                            "found module \"{}\", adding {} to module list...",
                            mod_name,
                            path.display()
                        ),
                        ParseUseMode::HiddenModulesInMyLib(_) => println!(
                            "found hidden module \"{}\", adding {} to module list...",
                            mod_name,
                            path.display()
                        ),
                    }
                }
                self.lib_modules.insert(mod_name, path.to_owned());
            }
        }
    }
    fn list_dependencies_of_my_lib(&self) -> BoxResult<Vec<String>> {
        // initialize blocked modules
        let mut dependencies = vec!["std".into()];

        // get dependencies from my_lib
        if let Some(ref my_lib) = self.my_lib {
            let my_lib_toml = fs::read_to_string(my_lib.parent().unwrap().join("Cargo.toml"))?
                .parse::<Value>()?;
            for (dep_name, _) in my_lib_toml
                .as_table()
                .unwrap()
                .get("dependencies")
                .unwrap()
                .as_table()
                .unwrap()
                .iter()
            {
                dependencies.push(dep_name.clone());
            }
        }

        Ok(dependencies)
    }
    fn get_lib_modules(&mut self) -> BoxResult<()> {
        // get lib modules if modules if not challenge_only and my_lib is specified
        if self.options.challenge_only {
            if self.options.verbose {
                println!(
                    "challenge_only -> skipping collecting path of all specified modules of lib..."
                );
            }
            return Ok(());
        }
        // get lib path
        let my_lib = match self.my_lib {
            Some(ref my_lib) => my_lib.clone(),
            None => {
                if self.options.verbose {
                    println!("lib \"{}\" not specified in toml -> skipping collecting path of all specified modules of lib...", self.options.lib);
                }
                return Ok(());
            }
        };
        // init local modules collection
        let mut lib_modules: BTreeMap<String, PathBuf> = BTreeMap::new();
        // insert lib.rs to lib_modules
        lib_modules.insert(self.options.lib.clone(), my_lib.join("lib.rs"));
        // parse modules of lib
        if self.options.verbose {
            println!("collecting all modules of \"{}\"...", self.options.lib);
        }
        self.parse_mod_from_src_file(
            my_lib.join("lib.rs"),
            self.options.lib.clone(),
            &mut lib_modules,
            false,
        )?;

        // get list of dependencies of my_lib
        let dependencies = self.list_dependencies_of_my_lib()?;

        // parse use statements in main.rs
        // create visitor from source code
        let visitor = SrcVisitor::new(&self.tmp_input_file)?;
        for use_item in visitor.uses.iter() {
            self.parse_use_item(
                &use_item.tree,
                "".into(),
                &ParseUseMode::InputCrate,
                &lib_modules,
            );
        }

        // parse use statements in local_modules
        let local_modules: Vec<PathBuf> =
            self.local_modules.values().map(|p| p.to_owned()).collect();
        for local_modules_path in local_modules.iter() {
            // create visitor from source code
            let visitor = SrcVisitor::new(local_modules_path)?;
            for use_item in visitor.uses.iter() {
                self.parse_use_item(
                    &use_item.tree,
                    "".into(),
                    &ParseUseMode::InputCrate,
                    &lib_modules,
                );
            }
        }

        // parse use statements in used lib modules
        let used_lib_modules = self.lib_modules.clone();
        for (mod_name, mod_path) in used_lib_modules.iter() {
            // create visitor from source code
            let visitor = SrcVisitor::new(mod_path)?;
            for use_item in visitor.uses.iter() {
                self.parse_use_item(
                    &use_item.tree,
                    mod_name.to_owned(),
                    &ParseUseMode::HiddenModulesInMyLib((
                        mod_name.to_owned(),
                        dependencies.clone(),
                    )),
                    &lib_modules,
                );
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
            .filter(|l| !l.trim().starts_with("use super::"))
            .filter(|l| !l.trim().starts_with("use self::"))
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
        for path in self.lib_modules.values() {
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
        files.insert("main.rs".into(), self.tmp_input_file.clone());
        for (_, file_input) in files.iter() {
            let mut input = String::new();
            self.load_challenge(file_input, &mut input)?;
            if self.options.verbose {
                println!("inserting {} into output...", file_input.display());
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
            for path in self.lib_modules.values() {
                self.load_lib(path.as_path(), &mut output)?;
            }
            for path in self.local_modules.values() {
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
            block_hidden: "my_compass;my_array".to_string(),
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
}

// generic analyze functions, which are available for all options,
// which fullfil the trait CliInput

use super::{AnalyzeError, AnalyzeState};
use crate::{add_context, configuration::CliInput, error::CgResult, CgData};

use anyhow::{anyhow, Context};
use cargo_metadata::{camino::Utf8PathBuf, Message};
use std::collections::BTreeMap;
use std::fmt::Write;

impl<O: CliInput> CgData<O, AnalyzeState> {
    fn get_input_path(&self) -> CgResult<Utf8PathBuf> {
        if self.options.verbose() {
            println!("Analyzing challenge code...");
        }

        // get bin name
        let bin_name = self.input_binary_name()?;

        // run 'cargo check' on bin_name to make sure, that input is ready to be processed
        let output = self.run_cargo_check_for_binary_of_root_package(bin_name)?;
        // collect any remaining messages
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

        // get bin path from metadata
        let input = self.get_binary_path_of_root_package(bin_name)?;

        if self.options.verbose() {
            println!("input src file: {}", input);
        }

        Ok(input.to_owned())
    }

    fn get_challenge_src_files(
        &self,
        input: &Utf8PathBuf,
    ) -> CgResult<BTreeMap<String, Utf8PathBuf>> {
        // parse modules of bin_crate
        if self.options.verbose() {
            println!(
                "collecting modules of bin_crate '{}'...",
                self.package_name()?
            );
        }
        // init challenge src file collection
        let mut challenge_src_files: BTreeMap<String, Utf8PathBuf> = BTreeMap::new();
        if self.options.verbose() {
            println!(
                "Input is '{}', adding {} to challenge src file list...",
                self.options.input().input,
                input,
            );
        }
        challenge_src_files.insert("main".into(), input.to_owned());
        // collect all modules of bin input crate
        parse_mod_from_src_file(
            input.to_owned(),
            "bin_crate".into(),
            &mut challenge_src_files,
            true,
        )?;

        // parse main.rs input file for crate lib.rs (use package_name::*;)
        // create visitor from source code
        let visitor = SrcVisitor::new(&input)?;
        let package_name = self.package_name()?;
        // check for use of package_name
        if visitor.uses.iter().any(|v| match &v.tree {
            UseTree::Path(use_path) => use_path.ident == package_name,
            _ => false,
        }) {
            // collecting modules of lib_crate
            if self.options.verbose() {
                println!(
                    "collecting modules of lib_crate '{}'...",
                    self.package_name()?
                );
            }
            // set path to lib.rs
            let lib_rs = self.package_src_dir()?.join("lib.rs");
            // add lib.rs to challenge_src_files
            if self.options.verbose() {
                println!(
                    "found module '{}', adding {} to challenge src file list...",
                    self.package_name()?,
                    lib_rs,
                );
            }
            // parse modules of lib_crate
            challenge_src_files.insert(self.package_name()?.to_owned(), lib_rs.clone());
            parse_mod_from_src_file(lib_rs, "lib_crate".into(), &mut challenge_src_files, true)?;
        }
        // return challenge_src_files
        Ok(challenge_src_files)
    }

    pub fn generic_analyze(&self) -> CgResult<BTreeMap<String, Utf8PathBuf>> {
        let input = self.get_input_path()?;

        let mut src_files = self.get_challenge_src_files(&input)?;
        let local_libraries = self.analyze_dependencies_of_package()?;
        Ok(src_files)
    }
}

// analyze specific helper functions

// Struct to visit source file and collect certain statements
use std::fs;
use syn::{visit::Visit, File, ItemMod, ItemUse, UseTree};
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
    fn new(path: &Utf8PathBuf) -> Result<SrcVisitor, AnalyzeError> {
        // load source code
        let code = fs::read_to_string(path)
            .context(add_context!("Unexpected failure of reading src file."))?;
        // Parse the source code into a syntax tree
        let syntax: File = syn::parse_file(&code).context(add_context!(
            "Unexpected failure of parsing src file content."
        ))?;
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

fn parse_mod_from_src_file(
    src_path: Utf8PathBuf,
    current_module: String,
    modules: &mut BTreeMap<String, Utf8PathBuf>,
    verbose: bool,
) -> Result<(), AnalyzeError> {
    // set directory, which contains the module src files, if there are some
    let current_mod_dir = match src_path
        .file_name()
        .context(add_context!("Unexpected missing file name"))?
    {
        "main.rs" | "lib.rs" | "mod.rs" => src_path
            .parent()
            .context(add_context!("Unexpected missing parent"))?
            .to_path_buf(),
        _ => {
            // check if input is main binary, but not main.rs
            if current_module == "bin_crate" {
                // src_path is input binary in ./src/bin
                src_path
                    .parent()
                    .context(add_context!("Unexpected missing parent"))?
                    .to_owned()
            } else {
                // src_path points to user defined module -> use file name as module name
                let mut current_mod_dir = src_path.clone();
                current_mod_dir.set_extension("");
                current_mod_dir
            }
        }
    };

    // if current_mod_dir does not exist, it cannot contain further modules.
    // therefore no parsing required
    if !current_mod_dir.is_dir() {
        return Ok(());
    }

    // create visitor from source code
    let visitor = SrcVisitor::new(&src_path)?;

    // parse mod entries, which are empty
    for item_mod in visitor.mods.iter().filter(|m| m.content.is_none()) {
        let mut module = item_mod.ident.to_string();
        // set module filename
        let mut path = current_mod_dir.join(module.clone() + ".rs");
        // set module name space path
        module = current_module.clone() + "::" + &module;
        // module is either 'module_name.rs' or 'module_name/mod.rs'
        if !path.is_file() {
            path.set_extension("");
            path = path.join("mod.rs");
            if !path.is_file() {
                Err(anyhow!(add_context!("Unexpected module file path error.")))?;
            }
        }

        if modules.insert(module.clone(), path.clone()).is_some() {
            // module already in collection
            continue;
        }
        if verbose {
            // ToDo: is this always challenge? Check later...
            println!(
                "found module '{}', adding {} to challenge src file list...",
                module, path
            );
        }
        parse_mod_from_src_file(path, module, modules, verbose)?;
    }
    Ok(())
}

// generic analyze functions, which are available for all options,
// which fullfil the trait CliInput

use super::{AnalyzeError, AnalyzeState};
use crate::{
    add_context,
    challenge_tree::{EdgeType, LocalPackage, NodeTyp},
    configuration::CliInput,
    error::CgResult,
    metadata::MetaWrapper,
    CgData,
};

use anyhow::{anyhow, Context};
use cargo_metadata::{camino::Utf8PathBuf, Message};
use clap::builder::Str;
use petgraph::graph::NodeIndex;
use quote::quote;
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
        let output = self
            .challenge_package()
            .metadata
            .run_cargo_check_for_binary_of_root_package(bin_name)?;
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
        let input = self
            .challenge_package()
            .metadata
            .get_binary_path_of_root_package(bin_name)?;

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
                self.challenge_package().name
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
        let package_name = self.challenge_package().name.to_owned();
        // check for use of package_name
        if visitor.uses.iter().any(|v| match &v.tree {
            UseTree::Path(use_path) => use_path.ident == package_name,
            _ => false,
        }) {
            // collecting modules of lib_crate
            if self.options.verbose() {
                println!(
                    "collecting modules of lib_crate '{}'...",
                    self.challenge_package().name
                );
            }
            // set path to lib.rs
            let lib_rs = self.challenge_package().path.join("lib.rs");
            // add lib.rs to challenge_src_files
            if self.options.verbose() {
                println!(
                    "found module '{}', adding {} to challenge src file list...",
                    self.challenge_package().name,
                    lib_rs,
                );
            }
            // parse modules of lib_crate
            challenge_src_files.insert(self.challenge_package().name.to_owned(), lib_rs.clone());
            parse_mod_from_src_file(lib_rs, "lib_crate".into(), &mut challenge_src_files, true)?;
        }
        // return challenge_src_files
        Ok(challenge_src_files)
    }

    fn analyze_dependencies_of_package(&mut self) -> Result<(), AnalyzeError> {
        let dependencies = self
            .challenge_package()
            .metadata
            .root_package()?
            .dependencies
            .to_owned();
        for dep in dependencies.iter() {
            if let Some(ref local_path) = dep.path {
                let dep_toml = local_path.join("Cargo.toml");
                let metadata = MetaWrapper::try_from(dep_toml)?;
                let dependency = LocalPackage::try_from(metadata)?;
                if self.options.verbose() {
                    println!(
                        "Found local dependency '{}' at '{}'",
                        dependency.name, dependency.path
                    );
                }
                // add dependency to tree
                let dep_index = self.tree.add_node(NodeTyp::LocalPackage(dependency));
                self.tree
                    .add_edge(0.into(), dep_index, EdgeType::Dependency);
            } else {
                let dep_name = dep.name.to_owned();
                if self.iter_supported_crates().any(|c| c == dep_name.as_str()) {
                    // found supported crate, add to tree
                    if self.options.verbose() {
                        println!("Found supported crate dependency '{}'", dep_name);
                    }
                    let supported_crate_index =
                        self.tree.add_node(NodeTyp::SupportedCrate(dep_name));
                    self.tree
                        .add_edge(0.into(), supported_crate_index, EdgeType::Dependency);
                } else {
                    return Err(AnalyzeError::CodingameUnsupportedDependencyOfChallenge(
                        dep_name,
                    ));
                }
            }
        }
        // check local dependencies for further dependencies
        let dependency_nodes: Vec<NodeIndex> =
            self.iter_local_dependencies().map(|(n, _)| n).collect();
        for dependency_node in dependency_nodes {
            self.analyze_dependencies_of_local_dependency(dependency_node)?;
        }
        Ok(())
    }

    fn analyze_dependencies_of_local_dependency(
        &mut self,
        node: NodeIndex,
    ) -> Result<(), AnalyzeError> {
        // get dependencies of local dependency
        let dependencies = self
            .get_local_dependency_package(node)?
            .metadata
            .root_package()
            .context(add_context!(
                "Unexpected missing root_package of dependency"
            ))?
            .dependencies
            .to_owned();
        // check dependencies of local dependency
        for dep in dependencies.iter() {
            if let Some(ref local_path) = dep.path {
                // if dependency is already in tree, get index of node or None.
                let dependency_node = self
                    .iter_local_dependencies()
                    .find(|(_, w)| w.name == dep.name)
                    .map(|(n, _)| n);
                // if Some(n), dependency is already in tree, therefore return node index, otherwise create new node
                // has to be done in two steps because of borrow checker
                let dependency_node = match dependency_node {
                    Some(n) => n,
                    None => {
                        let dep_toml = local_path.join("Cargo.toml");
                        let metadata = MetaWrapper::try_from(dep_toml)?;
                        let dependency = LocalPackage::try_from(metadata)?;
                        if self.options.verbose() {
                            println!(
                                "Found local dependency '{}' at '{}'",
                                dependency.name, dependency.path
                            );
                        }
                        self.tree.add_node(NodeTyp::LocalPackage(dependency))
                    }
                };
                self.tree
                    .add_edge(node, dependency_node, EdgeType::Dependency);
                // recursive call for checking dependencies of dependency
                self.analyze_dependencies_of_local_dependency(dependency_node)?;
            } else {
                let dep_name = dep.name.to_owned();
                if !self.iter_supported_crates().any(|c| c == dep_name) && !self.options.force() {
                    return Err(AnalyzeError::CodingameUnsupportedDependencyOfLocalLibrary(
                        dep_name,
                    ));
                }
                if !self
                    .iter_challenge_supported_crate_dependencies()
                    .any(|(_, c)| c == dep_name)
                    && !self.options.force()
                {
                    return Err(AnalyzeError::DependencyOfLocalLibraryIsNotIncludedInDependenciesOfChallenge(dep_name));
                }
            }
        }
        Ok(())
    }

    pub fn generic_analyze(&mut self) -> CgResult<BTreeMap<String, Utf8PathBuf>> {
        // add root package and dependencies to tree
        self.analyze_dependencies_of_package()?;

        let local_packages: Vec<&str> = self
            .iter_local_packages()
            .map(|(_, w)| w.name.as_str())
            .collect();
        println!("found local packages: {:?}", local_packages);

        let input = self.get_input_path()?;

        let mut src_files = self.get_challenge_src_files(&input)?;

        Ok(src_files)
    }
}

// analyze specific helper functions
use proc_macro2::TokenStream;
use std::fs;
use syn::{fold::Fold, visit::Visit, Attribute, File, ItemMod, ItemUse, Meta, UseTree};
struct AttrVisitor;

impl<'ast> Visit<'ast> for AttrVisitor {
    fn visit_attribute(&mut self, i: &'ast Attribute) {
        if let Meta::NameValue(attr) = &i.meta {
            if let Some(path) = attr.path.segments.last() {
                if path.ident.to_string() == "doc" {
                    println!("{:?}", attr.value);
                }
            }
        }
    }
}

struct AttrFoldRemoveDocComments;

impl Fold for AttrFoldRemoveDocComments {
    fn fold_attributes(&mut self, i: Vec<syn::Attribute>) -> Vec<syn::Attribute> {
        let attributes: Vec<syn::Attribute> = i
            .iter()
            .filter(|i| match &i.meta {
                Meta::NameValue(attr) => match attr.path.segments.last() {
                    // filter all doc comments
                    Some(path) => path.ident.to_string() != "doc",
                    None => true,
                },
                _ => true,
            })
            .map(|a| a.to_owned())
            .collect();
        attributes
    }
}
struct AttrFoldRemoveModTests;

impl Fold for AttrFoldRemoveModTests {
    fn fold_item(&mut self, i: syn::Item) -> syn::Item {
        match &i {
            syn::Item::Mod(mod_item) => {
                // remove tests module by replacing it with empty TokenStream
                if mod_item.ident.to_string() == "tests" {
                    syn::Item::Verbatim(TokenStream::new())
                } else {
                    i
                }
            }
            _ => i,
        }
    }
}

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
        // module is either 'module_name.rs' or 'module_name/mod.rs'
        if !path.is_file() {
            path.set_extension("");
            path = path.join("mod.rs");
            if !path.is_file() {
                Err(anyhow!(add_context!("Unexpected module file path error.")))?;
            }
        }

        // set module name space path
        module = current_module.clone() + "::" + &module;

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

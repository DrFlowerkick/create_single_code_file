// functions to interact with parsed src files

mod error;
pub use error::{ParsingError, ParsingResult};

use cargo_metadata::camino::Utf8PathBuf;
use proc_macro2::TokenStream;
use std::fs;
use syn::{fold::Fold, visit::Visit, Attribute, File, ItemMod, ItemUse, Meta, UseTree};

// load syntax from given file
pub fn load_syntax(path: &Utf8PathBuf) -> ParsingResult<File> {
    // load source code
    let code = fs::read_to_string(path)?;
    // Parse the source code into a syntax tree
    let syntax: File = syn::parse_file(&code)?;
    // remove doc comments
    let mut remove_doc_comments = FoldRemoveAttrDocComments;
    let syntax = remove_doc_comments.fold_file(syntax);
    // remove mod tests
    let mut remove_mod_tests = FoldRemoveItemModTests;
    let syntax = remove_mod_tests.fold_file(syntax);
    Ok(syntax)
}

// helper to remove doc comments from src file
struct FoldRemoveAttrDocComments;

impl Fold for FoldRemoveAttrDocComments {
    fn fold_attributes(&mut self, i: Vec<Attribute>) -> Vec<Attribute> {
        let attributes: Vec<Attribute> = i
            .iter()
            .filter(|i| match &i.meta {
                Meta::NameValue(attr) => match attr.path.segments.last() {
                    // filter all doc comments
                    Some(path) => path.ident != "doc",
                    None => true,
                },
                _ => true,
            })
            .map(|a| a.to_owned())
            .collect();
        attributes
    }
}

// helper to remove test modules from src files
struct FoldRemoveItemModTests;

impl Fold for FoldRemoveItemModTests {
    fn fold_item(&mut self, i: syn::Item) -> syn::Item {
        match &i {
            syn::Item::Mod(mod_item) => {
                // remove tests module by replacing it with empty TokenStream
                if mod_item.ident == "tests" {
                    syn::Item::Verbatim(TokenStream::new())
                } else {
                    i
                }
            }
            _ => i,
        }
    }
}

// Struct to visit source file and collect mod statements
#[derive(Default)]
pub struct ModVisitor {
    pub mods: Vec<ItemMod>,
}

impl<'ast> Visit<'ast> for ModVisitor {
    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        self.mods.push(i.clone());
    }
}

// Struct to visit source file and collect use statements
#[derive(Default)]
pub struct UseVisitor {
    pub uses: Vec<ItemUse>,
    external_dependencies: Vec<String>,
}

impl UseVisitor {
    pub fn new(mut external_dependencies: Vec<String>) -> Self {
        external_dependencies.push("std".into());
        external_dependencies.push("core".into());
        external_dependencies.push("alloc".into());
        Self {
            uses: Vec::new(),
            external_dependencies,
        }
    }
}

impl<'ast> Visit<'ast> for UseVisitor {
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        if let UseTree::Path(ref use_path) = i.tree {
            // filter external dependencies
            if self
                .external_dependencies
                .iter()
                .any(|fi| use_path.ident == fi)
            {
                return;
            }
        }
        self.uses.push(i.clone());
    }
}

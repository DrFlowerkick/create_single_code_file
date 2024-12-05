// functions to interact with parsed src files

mod error;
pub use error::{ParsingError, ParsingResult};

use cargo_metadata::camino::Utf8PathBuf;
use proc_macro2::TokenStream;
use std::fs;
use syn::{
    fold::Fold, visit::Visit, Attribute, File, Ident, Item, ItemUse, Meta, UseName, UseTree,
    Visibility,
};

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

// test for Group element in use tree
pub fn contains_use_group(use_tree: &UseTree) -> bool {
    match use_tree {
        UseTree::Path(use_path) => contains_use_group(&use_path.tree),
        UseTree::Glob(_) | UseTree::Name(_) | UseTree::Rename(_) => false,
        UseTree::Group(_) => true,
    }
}

// expand and collect use tree items from UseTree
pub fn get_use_items(use_tree: &UseTree) -> Vec<UseTree> {
    let mut use_trees: Vec<UseTree> = Vec::new();
    match use_tree {
        UseTree::Path(use_path) => {
            for sub_use_tree in get_use_items(&use_path.tree) {
                let mut new_path = use_path.to_owned();
                new_path.tree = Box::new(sub_use_tree);
                use_trees.push(UseTree::Path(new_path));
            }
        }
        UseTree::Group(use_group) => {
            for group_tree in use_group.items.iter() {
                for sub_use_tree in get_use_items(group_tree) {
                    use_trees.push(sub_use_tree);
                }
            }
        }
        UseTree::Glob(_) | UseTree::Name(_) | UseTree::Rename(_) => {
            use_trees.push(use_tree.to_owned());
        }
    }
    use_trees
}

// check if UseTree ends in glob; returns None if use statement contains groups
pub fn is_use_glob(use_tree: &UseTree) -> Option<bool> {
    match use_tree {
        UseTree::Path(use_path) => is_use_glob(&use_path.tree),
        UseTree::Group(_) => None,
        UseTree::Glob(_) => Some(true),
        UseTree::Name(_) | UseTree::Rename(_) => Some(false),
    }
}

// get first element name of path in use statement
pub fn get_start_of_use_path(use_tree: &UseTree) -> Option<String> {
    match use_tree {
        UseTree::Path(use_path) => Some(use_path.ident.to_string()),
        _ => None,
    }
}

// check visibility
fn is_visible(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

// get name of visible item
pub fn get_name_of_visible_item(item: &Item) -> Option<Ident> {
    match item {
        Item::Const(item_const) => {
            if is_visible(&item_const.vis) {
                Some(item_const.ident.to_owned())
            } else {
                None
            }
        }
        Item::Enum(item_enum) => {
            if is_visible(&item_enum.vis) {
                Some(item_enum.ident.to_owned())
            } else {
                None
            }
        }
        Item::ExternCrate(item_extern_crate) => {
            if is_visible(&item_extern_crate.vis) {
                Some(item_extern_crate.ident.to_owned())
            } else {
                None
            }
        }
        Item::Fn(item_fn) => {
            if is_visible(&item_fn.vis) {
                Some(item_fn.sig.ident.to_owned())
            } else {
                None
            }
        }
        Item::Mod(item_mod) => {
            if is_visible(&item_mod.vis) {
                Some(item_mod.ident.to_owned())
            } else {
                None
            }
        }
        Item::Static(item_static) => {
            if is_visible(&item_static.vis) {
                Some(item_static.ident.to_owned())
            } else {
                None
            }
        }
        Item::Struct(item_struct) => {
            if is_visible(&item_struct.vis) {
                Some(item_struct.ident.to_owned())
            } else {
                None
            }
        }
        Item::Trait(item_trait) => {
            if is_visible(&item_trait.vis) {
                Some(item_trait.ident.to_owned())
            } else {
                None
            }
        }
        Item::TraitAlias(item_trait_alias) => {
            if is_visible(&item_trait_alias.vis) {
                Some(item_trait_alias.ident.to_owned())
            } else {
                None
            }
        }
        Item::Type(item_type) => {
            if is_visible(&item_type.vis) {
                Some(item_type.ident.to_owned())
            } else {
                None
            }
        }
        Item::Union(item_union) => {
            if is_visible(&item_union.vis) {
                Some(item_union.ident.to_owned())
            } else {
                None
            }
        }
        Item::Use(item_use) => {
            if is_visible(&item_use.vis) {
                // expect expanded use tree (no group, no glob)
                let mut use_tree = &item_use.tree;
                'use_loop: loop {
                    match use_tree {
                        UseTree::Path(use_path) => use_tree = &use_path.tree,
                        UseTree::Group(_) | UseTree::Glob(_) => break 'use_loop None,
                        UseTree::Name(use_name) => break 'use_loop Some(use_name.ident.to_owned()),
                        UseTree::Rename(use_rename) => {
                            break 'use_loop Some(use_rename.rename.to_owned())
                        }
                    }
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

// replace glob with name
pub fn replace_glob_with_name(mut use_item: ItemUse, ident: Ident) -> Option<ItemUse> {
    let mut use_tree = &mut use_item.tree;
    loop {
        match use_tree {
            UseTree::Path(use_path) => use_tree = &mut use_path.tree,
            UseTree::Group(_) | UseTree::Name(_) | UseTree::Rename(_) => return None,
            UseTree::Glob(_) => {
                *use_tree = UseTree::Name(UseName { ident });
                return Some(use_item);
            }
        }
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

// functions to interact with parsed src files

mod error;
pub use error::{ParsingError, ParsingResult};

use crate::add_context;
use cargo_metadata::camino::Utf8PathBuf;
use quote::ToTokens;
use std::fmt::Write;
use std::fs;
use syn::{
    fold::Fold, visit::Visit, Attribute, File, Ident, ImplItem, Item, ItemUse, Meta, Type, UseName,
    UseTree, Visibility,
};

// load syntax from given file
pub fn load_syntax(path: &Utf8PathBuf) -> ParsingResult<File> {
    // load source code
    let code = fs::read_to_string(path)?;
    // Parse the source code into a syntax tree
    let syntax: File = syn::parse_file(&code)?;
    // remove doc comments
    let mut remove_doc_comments = FoldRemoveAttrDocComments;
    let mut syntax = remove_doc_comments.fold_file(syntax);
    // check for verbatim parsed elements
    let mut check_verbatim = VisitVerbatim {
        verbatim_tokens: String::new(),
    };
    check_verbatim.visit_file(&syntax);
    if !check_verbatim.verbatim_tokens.is_empty() {
        return Err(ParsingError::VerbatimError(check_verbatim.verbatim_tokens));
    }
    // remove mod tests and macros without a name
    syntax.items.retain(|item| match item {
        Item::Macro(item_macro) => item_macro.ident.is_some(),
        Item::Mod(item_mod) => item_mod.ident != "tests",
        _ => true,
    });
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

// helper to search for verbatim parsed code
struct VisitVerbatim {
    verbatim_tokens: String,
}

impl<'ast> Visit<'ast> for VisitVerbatim {
    fn visit_expr(&mut self, i: &'ast syn::Expr) {
        match i {
            syn::Expr::Verbatim(ts) => {
                write!(&mut self.verbatim_tokens, "{}\n", ts.to_token_stream()).expect(
                    &add_context!("Unexpected error when writing verbatim tokens."),
                );
            }
            _ => (),
        }
    }
    fn visit_item(&mut self, i: &'ast syn::Item) {
        match i {
            syn::Item::Verbatim(ts) => {
                write!(&mut self.verbatim_tokens, "{}\n", ts.to_token_stream()).expect(
                    &add_context!("Unexpected error when writing verbatim tokens."),
                );
            }
            _ => (),
        }
    }
    fn visit_foreign_item(&mut self, i: &'ast syn::ForeignItem) {
        match i {
            syn::ForeignItem::Verbatim(ts) => {
                write!(&mut self.verbatim_tokens, "{}\n", ts.to_token_stream()).expect(
                    &add_context!("Unexpected error when writing verbatim tokens."),
                );
            }
            _ => (),
        }
    }
    fn visit_trait_item(&mut self, i: &'ast syn::TraitItem) {
        match i {
            syn::TraitItem::Verbatim(ts) => {
                write!(&mut self.verbatim_tokens, "{}\n", ts.to_token_stream()).expect(
                    &add_context!("Unexpected error when writing verbatim tokens."),
                );
            }
            _ => (),
        }
    }
    fn visit_impl_item(&mut self, i: &'ast syn::ImplItem) {
        match i {
            syn::ImplItem::Verbatim(ts) => {
                write!(&mut self.verbatim_tokens, "{}\n", ts.to_token_stream()).expect(
                    &add_context!("Unexpected error when writing verbatim tokens."),
                );
            }
            _ => (),
        }
    }
    fn visit_lit(&mut self, i: &'ast syn::Lit) {
        match i {
            syn::Lit::Verbatim(ts) => {
                write!(&mut self.verbatim_tokens, "{}\n", ts.to_token_stream()).expect(
                    &add_context!("Unexpected error when writing verbatim tokens."),
                );
            }
            _ => (),
        }
    }
    fn visit_pat(&mut self, i: &'ast syn::Pat) {
        match i {
            syn::Pat::Verbatim(ts) => {
                write!(&mut self.verbatim_tokens, "{}\n", ts.to_token_stream()).expect(
                    &add_context!("Unexpected error when writing verbatim tokens."),
                );
            }
            _ => (),
        }
    }
    fn visit_type(&mut self, i: &'ast syn::Type) {
        match i {
            syn::Type::Verbatim(ts) => {
                write!(&mut self.verbatim_tokens, "{}\n", ts.to_token_stream()).expect(
                    &add_context!("Unexpected error when writing verbatim tokens."),
                );
            }
            _ => (),
        }
    }
    fn visit_type_param_bound(&mut self, i: &'ast syn::TypeParamBound) {
        match i {
            syn::TypeParamBound::Verbatim(ts) => {
                write!(&mut self.verbatim_tokens, "{}\n", ts.to_token_stream()).expect(
                    &add_context!("Unexpected error when writing verbatim tokens."),
                );
            }
            _ => (),
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
        Item::ForeignMod(_) | Item::Impl(_) | Item::Macro(_) | Item::Verbatim(_) => None,
        _ => None, // Item is #[non_exhaustive]
    }
}

// replace glob with name
pub fn replace_glob_with_ident(mut use_item: ItemUse, ident: Ident) -> Option<ItemUse> {
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

// check impl name with single path element
pub fn first_item_impl_is_ident<I>(item: &Item, ident: &I) -> bool
where
    I: ?Sized + std::fmt::Debug,
    Ident: PartialEq<I>,
{
    if let Item::Impl(item_impl) = item {
        if let Type::Path(type_path) = item_impl.self_ty.as_ref() {
            if let Some(first_ident) = type_path.path.segments.first() {
                return first_ident.ident == *ident;
            }
        }
    }
    false
}

// get name of item
pub fn get_name_of_item(item: &Item) -> Option<(String, String)> {
    match item {
        Item::Const(item_const) => Some(("Const".into(), item_const.ident.to_string())),
        Item::Enum(item_enum) => Some(("Enum".into(), item_enum.ident.to_string())),
        Item::ExternCrate(item_extern_crate) => {
            Some(("ExternCrate".into(), item_extern_crate.ident.to_string()))
        }
        Item::Fn(item_fn) => Some(("Fn".into(), item_fn.sig.ident.to_string())),
        Item::Macro(item_macro) => item_macro
            .ident
            .as_ref()
            .map(|i| ("Macro".into(), i.to_string())),
        Item::Mod(item_mod) => Some(("Mod".into(), item_mod.ident.to_string())),
        Item::Static(item_static) => Some(("Static".into(), item_static.ident.to_string())),
        Item::Struct(item_struct) => Some(("Struct".into(), item_struct.ident.to_string())),
        Item::Trait(item_trait) => Some(("Trait".into(), item_trait.ident.to_string())),
        Item::TraitAlias(item_trait_alias) => {
            Some(("TraitAlias".into(), item_trait_alias.ident.to_string()))
        }
        Item::Type(item_type) => Some(("Type".into(), item_type.ident.to_string())),
        Item::Union(item_union) => Some(("Union".into(), item_union.ident.to_string())),
        Item::Use(item_use) => {
            // expect expanded use tree (no group, no glob)
            let mut use_tree = &item_use.tree;
            'use_loop: loop {
                match use_tree {
                    UseTree::Path(use_path) => use_tree = &use_path.tree,
                    UseTree::Group(_) | UseTree::Glob(_) => break 'use_loop None,
                    UseTree::Name(use_name) => {
                        break 'use_loop Some(("Use".into(), use_name.ident.to_string()))
                    }
                    UseTree::Rename(use_rename) => {
                        break 'use_loop Some(("Use".into(), use_rename.rename.to_string()))
                    }
                }
            }
        }
        Item::ForeignMod(_) => Some(("ForeignMod".into(), "NAMELESS".into())),
        Item::Impl(item_impl) => if let Some((_, ref trait_, _)) = item_impl.trait_ {
            Some(("Impl".into(), trait_.to_token_stream().to_string()))
        } else {
            Some(("Impl".into(), "".into()))
        },
        Item::Verbatim(_) => Some(("Verbatim".into(), "NAMELESS".into())),
        _ => None, // Item is #[non_exhaustive]
    }
}

// get name of impl item
pub fn get_name_of_impl_item(impl_item: &ImplItem) -> Option<(String, String)> {
    match impl_item {
        ImplItem::Const(impl_item_const) => {
            Some(("Const".into(), impl_item_const.ident.to_string()))
        }
        ImplItem::Fn(impl_item_fn) => Some(("Fn".into(), impl_item_fn.sig.ident.to_string())),
        ImplItem::Macro(impl_item_macro) => Some((
            "Macro".into(),
            impl_item_macro.mac.path.to_token_stream().to_string(),
        )),
        ImplItem::Type(impl_item_type) => Some(("Type".into(), impl_item_type.ident.to_string())),
        ImplItem::Verbatim(_) => Some(("Verbatim".into(), "NAMELESS".into())),
        _ => None,
    }
}

// struct to visit types
#[derive(Default)]
pub struct TypeVisitor {
    pub types: Vec<Type>,
}

impl<'ast> Visit<'ast> for TypeVisitor {
    fn visit_type(&mut self, i: &'ast syn::Type) {
        self.types.push(i.to_owned());
        syn::visit::visit_type(self, i);
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

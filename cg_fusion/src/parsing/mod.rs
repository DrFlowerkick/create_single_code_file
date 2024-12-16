// functions to interact with parsed src files

mod error;
pub use error::{ParsingError, ParsingResult};

use crate::add_context;
use cargo_metadata::camino::Utf8PathBuf;
use quote::ToTokens;
use std::fmt::{Display, Write};
use std::fs;
use syn::{
    fold::Fold, visit::Visit, Attribute, File, Ident, ImplItem, Item, ItemUse, Meta, Path, Type,
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
    fn fold_attributes(&mut self, mut i: Vec<Attribute>) -> Vec<Attribute> {
        i.retain(|attr| match &attr.meta {
            Meta::NameValue(mnv) => match mnv.path.segments.last() {
                // filter all doc comments
                Some(path) => path.ident != "doc",
                None => true,
            },
            _ => true,
        });
        i
    }
}

// helper to search for verbatim parsed code
struct VisitVerbatim {
    verbatim_tokens: String,
}

impl<'ast> Visit<'ast> for VisitVerbatim {
    fn visit_expr(&mut self, i: &'ast syn::Expr) {
        if let syn::Expr::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_item(&mut self, i: &'ast syn::Item) {
        if let syn::Item::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_foreign_item(&mut self, i: &'ast syn::ForeignItem) {
        if let syn::ForeignItem::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_trait_item(&mut self, i: &'ast syn::TraitItem) {
        if let syn::TraitItem::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_impl_item(&mut self, i: &'ast syn::ImplItem) {
        if let syn::ImplItem::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_lit(&mut self, i: &'ast syn::Lit) {
        if let syn::Lit::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_pat(&mut self, i: &'ast syn::Pat) {
        if let syn::Pat::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_type(&mut self, i: &'ast syn::Type) {
        if let syn::Type::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
        }
    }
    fn visit_type_param_bound(&mut self, i: &'ast syn::TypeParamBound) {
        if let syn::TypeParamBound::Verbatim(ts) = i {
            writeln!(&mut self.verbatim_tokens, "{}", ts.to_token_stream()).expect(&add_context!(
                "Unexpected error when writing verbatim tokens."
            ));
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

// path analysis
pub trait PathAnalysis {
    fn extract_path(&self) -> Option<SourcePath>;
}

pub struct SourcePath {
    pub segments: Vec<Ident>,
    pub glob: bool,
    pub rename: Option<Ident>,
}

impl PathAnalysis for UseTree {
    fn extract_path(&self) -> Option<SourcePath> {
        let mut tree = self;
        let mut segments: Vec<Ident> = Vec::new();
        loop {
            match tree {
                UseTree::Path(use_path) => {
                    segments.push(use_path.ident.to_owned());
                    tree = &use_path.tree;
                }
                UseTree::Group(_) => return None,
                UseTree::Glob(_) => {
                    return Some(SourcePath {
                        segments,
                        glob: true,
                        rename: None,
                    })
                }
                UseTree::Name(use_name) => {
                    segments.push(use_name.ident.to_owned());
                    return Some(SourcePath {
                        segments,
                        glob: false,
                        rename: None,
                    });
                }
                UseTree::Rename(use_rename) => {
                    segments.push(use_rename.ident.to_owned());
                    return Some(SourcePath {
                        segments,
                        glob: false,
                        rename: Some(use_rename.rename.to_owned()),
                    });
                }
            }
        }
    }
}

impl PathAnalysis for Path {
    fn extract_path(&self) -> Option<SourcePath> {
        Some(SourcePath {
            segments: self.segments.iter().map(|s| s.ident.to_owned()).collect(),
            glob: false,
            rename: None,
        })
    }
}

// check if UseTree ends in glob; returns None if use statement contains groups
pub fn is_use_glob(item: &Item) -> Option<bool> {
    if let Item::Use(use_item) = item {
        let mut use_tree = &use_item.tree;
        loop {
            match use_tree {
                UseTree::Path(use_path) => use_tree = &use_path.tree,
                UseTree::Group(_) => return None,
                UseTree::Glob(_) => return Some(true),
                UseTree::Name(_) | UseTree::Rename(_) => return Some(false),
            }
        }
    }
    None
}

// extract visibility
pub fn extract_visibility(item: &Item) -> Option<&Visibility> {
    match item {
        Item::Const(item_const) => Some(&item_const.vis),
        Item::Enum(item_enum) => Some(&item_enum.vis),
        Item::ExternCrate(item_extern_crate) => Some(&item_extern_crate.vis),
        Item::Fn(item_fn) => Some(&item_fn.vis),
        Item::Mod(item_mod) => Some(&item_mod.vis),
        Item::Static(item_static) => Some(&item_static.vis),
        Item::Struct(item_struct) => Some(&item_struct.vis),
        Item::Trait(item_trait) => Some(&item_trait.vis),
        Item::TraitAlias(item_trait_alias) => Some(&item_trait_alias.vis),
        Item::Type(item_type) => Some(&item_type.vis),
        Item::Union(item_union) => Some(&item_union.vis),
        Item::Use(item_use) => Some(&item_use.vis),
        _ => None, // all other items don't have a visibility attribute
    }
}

// replace glob with last path element of visible use tree
pub fn replace_glob_with_name_or_rename_use_tree(
    mut use_item: ItemUse,
    replace: UseTree,
) -> Option<ItemUse> {
    match replace {
        UseTree::Glob(_) | UseTree::Path(_) | UseTree::Group(_) => return None,
        UseTree::Name(_) | UseTree::Rename(_) => (),
    }
    let mut use_tree = &mut use_item.tree;
    loop {
        match use_tree {
            UseTree::Path(use_path) => use_tree = &mut use_path.tree,
            UseTree::Group(_) | UseTree::Name(_) | UseTree::Rename(_) => return None,
            UseTree::Glob(_) => {
                *use_tree = replace;
                return Some(use_item);
            }
        }
    }
}

// check impl name with first path element
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
// check impl trait with first path element
pub fn first_trait_impl_is_ident<I>(item: &Item, ident: &I) -> bool
where
    I: ?Sized + std::fmt::Debug,
    Ident: PartialEq<I>,
{
    if let Item::Impl(item_impl) = item {
        if let Some((_, ref trait_path, _)) = item_impl.trait_ {
            if let Some(first_ident) = trait_path.segments.first() {
                return first_ident.ident == *ident;
            }
        }
    }
    false
}

#[derive(Debug)]
pub enum ItemName {
    TypeStringAndIdent(String, Ident),
    TypeStringAndRenamed(String, Ident, Ident),
    TypeStringAndNameString(String, String),
    TypeString(String),
    None,
}

impl Display for ItemName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TypeStringAndIdent(ts, i) => write!(f, "{:?} ({ts})", i),
            Self::TypeStringAndRenamed(ts, i, r) => write!(f, "{:?} as {:?} ({ts})", i, r),
            Self::TypeStringAndNameString(ts, ns) => write!(f, "{ns} ({ts})"),
            Self::TypeString(ts) => write!(f, "({ts})"),
            Self::None => write!(f, "(UNKNOWN)"),
        }
    }
}

impl ItemName {
    pub fn extract_ident(&self) -> Option<Ident> {
        match self {
            Self::TypeStringAndIdent(_, ident) => Some(ident.to_owned()),
            Self::TypeStringAndRenamed(_, ident, _) => Some(ident.to_owned()),
            _ => None,
        }
    }
    pub fn extract_name(&self) -> Option<String> {
        match self {
            Self::TypeStringAndIdent(_, ident) => Some(ident.to_string()),
            Self::TypeStringAndRenamed(_, ident, _) => Some(ident.to_string()),
            Self::TypeStringAndNameString(_, name) => Some(name.to_owned()),
            _ => None,
        }
    }
    pub fn extract_rename(&self) -> Option<Ident> {
        match self {
            Self::TypeStringAndRenamed(_, _, rename) => Some(rename.to_owned()),
            _ => None,
        }
    }
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

// get name of item
pub fn get_name_of_item(item: &Item) -> ItemName {
    match item {
        Item::Const(item_const) => {
            ItemName::TypeStringAndIdent("Const".into(), item_const.ident.to_owned())
        }
        Item::Enum(item_enum) => {
            ItemName::TypeStringAndIdent("Enum".into(), item_enum.ident.to_owned())
        }
        Item::ExternCrate(item_extern_crate) => {
            ItemName::TypeStringAndIdent("ExternCrate".into(), item_extern_crate.ident.to_owned())
        }
        Item::Fn(item_fn) => {
            ItemName::TypeStringAndIdent("Fn".into(), item_fn.sig.ident.to_owned())
        }
        Item::Macro(item_macro) => match item_macro.ident {
            Some(ref ident) => ItemName::TypeStringAndIdent("Macro".into(), ident.to_owned()),
            None => ItemName::None,
        },
        Item::Mod(item_mod) => {
            ItemName::TypeStringAndIdent("Mod".into(), item_mod.ident.to_owned())
        }
        Item::Static(item_static) => {
            ItemName::TypeStringAndIdent("Static".into(), item_static.ident.to_owned())
        }
        Item::Struct(item_struct) => {
            ItemName::TypeStringAndIdent("Struct".into(), item_struct.ident.to_owned())
        }
        Item::Trait(item_trait) => {
            ItemName::TypeStringAndIdent("Trait".into(), item_trait.ident.to_owned())
        }
        Item::TraitAlias(item_trait_alias) => {
            ItemName::TypeStringAndIdent("TraitAlias".into(), item_trait_alias.ident.to_owned())
        }
        Item::Type(item_type) => {
            ItemName::TypeStringAndIdent("Type".into(), item_type.ident.to_owned())
        }
        Item::Union(item_union) => {
            ItemName::TypeStringAndIdent("Union".into(), item_union.ident.to_owned())
        }
        Item::Use(item_use) => {
            // expect expanded use tree (no group, no glob)
            let mut use_tree = &item_use.tree;
            'use_loop: loop {
                match use_tree {
                    UseTree::Path(use_path) => use_tree = &use_path.tree,
                    UseTree::Group(_) | UseTree::Glob(_) => break 'use_loop ItemName::None,
                    UseTree::Name(use_name) => {
                        break 'use_loop ItemName::TypeStringAndIdent(
                            "Use".into(),
                            use_name.ident.to_owned(),
                        )
                    }
                    UseTree::Rename(use_rename) => {
                        break 'use_loop ItemName::TypeStringAndRenamed(
                            "Use".into(),
                            use_rename.ident.to_owned(),
                            use_rename.rename.to_owned(),
                        )
                    }
                }
            }
        }
        Item::ForeignMod(_) => ItemName::TypeString("ForeignMod".into()),
        Item::Impl(item_impl) => {
            if let Some((_, ref trait_, _)) = item_impl.trait_ {
                ItemName::TypeStringAndNameString(
                    "Impl".into(),
                    trait_.to_token_stream().to_string(),
                )
            } else {
                ItemName::TypeString("Impl".into())
            }
        }
        Item::Verbatim(_) => ItemName::TypeString("Verbatim".into()),
        _ => ItemName::None, // Item is #[non_exhaustive]
    }
}

// get name of impl item
pub fn get_name_of_impl_item(impl_item: &ImplItem) -> ItemName {
    match impl_item {
        ImplItem::Const(impl_item_const) => {
            ItemName::TypeStringAndIdent("Const".into(), impl_item_const.ident.to_owned())
        }
        ImplItem::Fn(impl_item_fn) => {
            ItemName::TypeStringAndIdent("Fn".into(), impl_item_fn.sig.ident.to_owned())
        }
        ImplItem::Macro(impl_item_macro) => ItemName::TypeStringAndNameString(
            "Macro".into(),
            impl_item_macro.mac.path.to_token_stream().to_string(),
        ),
        ImplItem::Type(impl_item_type) => {
            ItemName::TypeStringAndIdent("Type".into(), impl_item_type.ident.to_owned())
        }
        ImplItem::Verbatim(_) => ItemName::TypeString("Verbatim".into()),
        _ => ItemName::None,
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

// functions to interact with parsed src files

mod error;
pub use error::{ParsingError, ParsingResult};
use syn::UseName;

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

// path analysis
pub trait PathAnalysis {
    fn extract_path(&self) -> SourcePath;
    fn extract_path_root(&self) -> Ident;
}

#[derive(Debug)]
pub enum SourcePath {
    Name(Vec<Ident>),
    Glob(Vec<Ident>),
    Rename(Vec<Ident>, Ident),
    Group,
}

impl SourcePath {
    pub fn get_last(&self) -> Option<&Ident> {
        match self {
            SourcePath::Name(segments)
            | SourcePath::Glob(segments)
            | SourcePath::Rename(segments, _) => segments.last(),
            SourcePath::Group => None,
        }
    }
}

impl PathAnalysis for UseTree {
    fn extract_path(&self) -> SourcePath {
        let mut tree = self;
        let mut segments: Vec<Ident> = Vec::new();
        loop {
            match tree {
                UseTree::Path(use_path) => {
                    segments.push(use_path.ident.to_owned());
                    tree = &use_path.tree;
                }
                UseTree::Group(_) => return SourcePath::Group,
                UseTree::Glob(_) => return SourcePath::Glob(segments),
                UseTree::Name(use_name) => {
                    segments.push(use_name.ident.to_owned());
                    return SourcePath::Name(segments);
                }
                UseTree::Rename(use_rename) => {
                    segments.push(use_rename.ident.to_owned());
                    return SourcePath::Rename(segments, use_rename.rename.to_owned());
                }
            }
        }
    }

    fn extract_path_root(&self) -> Ident {
        match self {
            UseTree::Path(use_path) => use_path.ident.to_owned(),
            UseTree::Group(_) | UseTree::Glob(_) => {
                unreachable!("UseTree cannot start with group or glob.")
            }
            UseTree::Name(name) => name.ident.to_owned(),
            UseTree::Rename(rename) => rename.rename.to_owned(),
        }
    }
}

impl PathAnalysis for Path {
    fn extract_path(&self) -> SourcePath {
        SourcePath::Name(self.segments.iter().map(|s| s.ident.to_owned()).collect())
    }
    fn extract_path_root(&self) -> Ident {
        self.segments.first().unwrap().ident.to_owned()
    }
}

trait UseTreeExtras {
    fn get_use_items_of_use_group(&self) -> Vec<UseTree>;
}

impl UseTreeExtras for UseTree {
    fn get_use_items_of_use_group(&self) -> Vec<UseTree> {
        let mut use_trees: Vec<UseTree> = Vec::new();
        match self {
            UseTree::Path(use_path) => {
                for sub_use_tree in use_path.tree.get_use_items_of_use_group() {
                    let mut new_path = use_path.to_owned();
                    new_path.tree = Box::new(sub_use_tree);
                    use_trees.push(UseTree::Path(new_path));
                }
            }
            UseTree::Group(use_group) => {
                for group_tree in use_group.items.iter() {
                    for sub_use_tree in group_tree.get_use_items_of_use_group() {
                        use_trees.push(sub_use_tree);
                    }
                }
            }
            UseTree::Glob(_) | UseTree::Name(_) | UseTree::Rename(_) => {
                use_trees.push(self.to_owned());
            }
        }
        use_trees
    }
}

pub trait ItemExtras {
    fn contains_use_group(&self) -> bool;
    fn get_use_items_of_use_group(&self) -> Vec<Item>;
    fn get_item_use(&self) -> Option<&ItemUse>;
    fn is_use_glob(&self) -> Option<&UseTree>;
    fn extract_visibility(&self) -> Option<&Visibility>;
    fn replace_glob_with_name_ident(self, ident: Ident) -> Option<Item>;
}

impl ItemExtras for Item {
    fn contains_use_group(&self) -> bool {
        if let Item::Use(item_use) = self {
            let mut tree = &item_use.tree;
            loop {
                match tree {
                    UseTree::Path(use_path) => tree = use_path.tree.as_ref(),
                    UseTree::Group(_) => return true,
                    UseTree::Glob(_) | UseTree::Name(_) | UseTree::Rename(_) => return false,
                }
            }
        }
        false
    }

    fn get_use_items_of_use_group(&self) -> Vec<Item> {
        if let Item::Use(item_use) = self {
            let new_items: Vec<Item> = item_use
                .tree
                .get_use_items_of_use_group()
                .iter()
                .map(|u| {
                    let mut new_item_use = item_use.clone();
                    new_item_use.tree = u.to_owned();
                    Item::Use(new_item_use)
                })
                .collect();
            return new_items;
        }
        Vec::new()
    }

    fn get_item_use(&self) -> Option<&ItemUse> {
        if let Item::Use(item_use) = self {
            return Some(item_use);
        }
        None
    }

    fn is_use_glob(&self) -> Option<&UseTree> {
        if let Item::Use(item_use) = self {
            return if let SourcePath::Glob(_) = item_use.tree.extract_path() {
                Some(&item_use.tree)
            } else {
                None
            };
        }
        None
    }

    fn extract_visibility(&self) -> Option<&Visibility> {
        match self {
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

    fn replace_glob_with_name_ident(mut self, ident: Ident) -> Option<Item> {
        if let Item::Use(ref mut item_use) = self {
            let mut use_tree = &mut item_use.tree;
            loop {
                match use_tree {
                    UseTree::Path(use_path) => use_tree = &mut use_path.tree,
                    UseTree::Group(_) | UseTree::Name(_) | UseTree::Rename(_) => return None,
                    UseTree::Glob(_) => {
                        let name = UseTree::Name(UseName { ident });
                        *use_tree = name;
                        return Some(self);
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub enum ItemName {
    TypeStringAndIdent(String, Ident),
    TypeStringAndRenamed(String, Ident, Ident),
    TypeStringAndNameString(String, String),
    TypeStringAndTraitAndNameString(String, Ident, String),
    TypeString(String),
    Group,
    Glob,
    None,
}

impl Display for ItemName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TypeStringAndIdent(ts, i) => write!(f, "{:?} ({ts})", i),
            Self::TypeStringAndRenamed(ts, i, r) => write!(f, "{:?} as {:?} ({ts})", i, r),
            Self::TypeStringAndNameString(ts, ns) => write!(f, "{ns} ({ts})"),
            Self::TypeStringAndTraitAndNameString(ts, t, ns) => {
                write!(f, "{:?} for {ns} ({ts})", t)
            }
            Self::TypeString(ts) => write!(f, "({ts})"),
            Self::Group => write!(f, "(group)"),
            Self::Glob => write!(f, "(glob *)"),
            Self::None => write!(f, "(UNKNOWN)"),
        }
    }
}

impl ItemName {
    pub fn get_ident_in_name_space(&self) -> Option<Ident> {
        match self {
            Self::TypeStringAndIdent(_, ident) => Some(ident.to_owned()),
            Self::TypeStringAndRenamed(_, _, rename) => Some(rename.to_owned()),
            _ => None,
        }
    }
    pub fn get_name(&self) -> Option<String> {
        match self {
            Self::TypeStringAndIdent(_, ident) => Some(ident.to_string()),
            Self::TypeStringAndRenamed(_, ident, _) => Some(ident.to_string()),
            Self::TypeStringAndNameString(_, name) => Some(name.to_owned()),
            _ => None,
        }
    }
}

impl From<&Item> for ItemName {
    fn from(item: &Item) -> Self {
        match item {
            Item::Const(item_const) => {
                ItemName::TypeStringAndIdent("Const".into(), item_const.ident.to_owned())
            }
            Item::Enum(item_enum) => {
                ItemName::TypeStringAndIdent("Enum".into(), item_enum.ident.to_owned())
            }
            Item::ExternCrate(item_extern_crate) => ItemName::TypeStringAndIdent(
                "ExternCrate".into(),
                item_extern_crate.ident.to_owned(),
            ),
            Item::Fn(item_fn) => {
                ItemName::TypeStringAndIdent("Fn".into(), item_fn.sig.ident.to_owned())
            }
            Item::ForeignMod(_) => ItemName::TypeString("ForeignMod".into()),
            Item::Impl(item_impl) => {
                let trait_ident: Option<Ident> = if let Some((_, ref trait_, _)) = item_impl.trait_
                {
                    trait_.extract_path().get_last().map(|i| i.to_owned())
                } else {
                    None
                };
                match item_impl.self_ty.as_ref() {
                    // at current state of code, we only support Path and Reference
                    Type::Path(type_path) => {
                        let path_target = match type_path.path.extract_path().get_last() {
                            Some(ident) => ident.to_owned(),
                            None => unreachable!("Path must have at least one segment."),
                        };
                        if let Some(ti) = trait_ident {
                            ItemName::TypeStringAndTraitAndNameString(
                                "Impl".into(),
                                ti,
                                path_target.to_string(),
                            )
                        } else {
                            ItemName::TypeStringAndNameString(
                                "Impl".into(),
                                path_target.to_string(),
                            )
                        }
                    }
                    Type::Reference(type_ref) => {
                        if let Type::Path(type_path) = type_ref.elem.as_ref() {
                            let path_target = match type_path.path.extract_path().get_last() {
                                Some(ident) => ident.to_owned(),
                                None => unreachable!("Path must have at least one segment."),
                            };
                            if let Some(ti) = trait_ident {
                                ItemName::TypeStringAndTraitAndNameString(
                                    "Impl".into(),
                                    ti,
                                    path_target.to_string(),
                                )
                            } else {
                                ItemName::TypeStringAndNameString(
                                    "Impl".into(),
                                    path_target.to_string(),
                                )
                            }
                        } else {
                            ItemName::TypeString("Impl".into())
                        }
                    }
                    _ => {
                        if let Some(ti) = trait_ident {
                            ItemName::TypeStringAndTraitAndNameString(
                                "Impl".into(),
                                ti,
                                item_impl.to_token_stream().to_string(),
                            )
                        } else {
                            ItemName::TypeStringAndNameString(
                                "Impl".into(),
                                item_impl.to_token_stream().to_string(),
                            )
                        }
                    }
                }
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
            Item::Use(item_use) => match item_use.tree.extract_path() {
                SourcePath::Group => ItemName::Group,
                SourcePath::Glob(_) => ItemName::Glob,
                SourcePath::Name(segments) => {
                    ItemName::TypeStringAndIdent("Use".into(), segments.last().unwrap().to_owned())
                }
                SourcePath::Rename(segments, rename) => ItemName::TypeStringAndRenamed(
                    "Use".into(),
                    segments.last().unwrap().to_owned(),
                    rename.to_owned(),
                ),
            },
            Item::Verbatim(_) => ItemName::TypeString("Verbatim".into()),
            _ => ItemName::None, // Item is #[non_exhaustive]
        }
    }
}

impl From<&ImplItem> for ItemName {
    fn from(impl_item: &ImplItem) -> Self {
        match impl_item {
            ImplItem::Const(impl_item_const) => {
                ItemName::TypeStringAndIdent("Impl Const".into(), impl_item_const.ident.to_owned())
            }
            ImplItem::Fn(impl_item_fn) => {
                ItemName::TypeStringAndIdent("Impl Fn".into(), impl_item_fn.sig.ident.to_owned())
            }
            ImplItem::Macro(impl_item_macro) => {
                match impl_item_macro.mac.path.extract_path().get_last() {
                    Some(ident) => {
                        ItemName::TypeStringAndIdent("Impl Macro".into(), ident.to_owned())
                    }
                    None => ItemName::TypeString("Impl Macro".into()),
                }
            }
            ImplItem::Type(impl_item_type) => {
                ItemName::TypeStringAndIdent("Impl Type".into(), impl_item_type.ident.to_owned())
            }
            ImplItem::Verbatim(_) => ItemName::TypeString("Impl Verbatim".into()),
            _ => ItemName::None,
        }
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

// Struct to visit syn items and check, if ident is used in item
pub struct IdentVisitor {
    pub ident: Ident,
    pub found: bool,
}

impl IdentVisitor {
    pub fn new(ident: Ident) -> Self {
        Self {
            ident,
            found: false,
        }
    }
}

impl<'ast> Visit<'ast> for IdentVisitor {
    fn visit_ident(&mut self, i: &'ast syn::Ident) {
        if i == &self.ident {
            self.found = true;
        }
    }
}

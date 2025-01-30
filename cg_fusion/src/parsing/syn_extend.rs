// extend syn to fit needs of cg_fusion

use super::{ItemName, ParsingError};
use crate::utilities::Sortable;
use syn::{
    ExprPath, ExprStruct, Ident, ImplItemMacro, Item, ItemUse, Path, TraitItemMacro, TypePath,
    UseGlob, UseName, UsePath, UseRename, UseTree, VisRestricted, Visibility,
};

#[derive(Debug, Clone)]
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

impl From<&UseTree> for SourcePath {
    fn from(mut use_tree: &UseTree) -> Self {
        let mut segments: Vec<Ident> = Vec::new();
        loop {
            match use_tree {
                UseTree::Path(use_path) => {
                    segments.push(use_path.ident.to_owned());
                    use_tree = &use_path.tree;
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
}

impl From<&Path> for SourcePath {
    fn from(path: &Path) -> Self {
        SourcePath::Name(path.segments.iter().map(|s| s.ident.to_owned()).collect())
    }
}

impl From<&ItemUse> for SourcePath {
    fn from(item_use: &ItemUse) -> Self {
        SourcePath::from(&item_use.tree)
    }
}

impl From<&TypePath> for SourcePath {
    fn from(type_path: &TypePath) -> Self {
        SourcePath::from(&type_path.path)
    }
}

impl From<&ExprStruct> for SourcePath {
    fn from(expr_struct: &ExprStruct) -> Self {
        SourcePath::from(&expr_struct.path)
    }
}

impl From<&ExprPath> for SourcePath {
    fn from(expr_path: &ExprPath) -> Self {
        SourcePath::from(&expr_path.path)
    }
}

impl From<&ImplItemMacro> for SourcePath {
    fn from(impl_item_macro: &ImplItemMacro) -> Self {
        SourcePath::from(&impl_item_macro.mac.path)
    }
}

impl From<&TraitItemMacro> for SourcePath {
    fn from(trait_item_macro: &TraitItemMacro) -> Self {
        SourcePath::from(&trait_item_macro.mac.path)
    }
}

impl From<&VisRestricted> for SourcePath {
    fn from(vis_restricted: &VisRestricted) -> Self {
        SourcePath::from(vis_restricted.path.as_ref())
    }
}

impl TryFrom<SourcePath> for UseTree {
    type Error = ParsingError;

    fn try_from(value: SourcePath) -> Result<Self, Self::Error> {
        let (mut use_tree, segments) = match value {
            SourcePath::Name(ref segments) => {
                let last_segment = segments
                    .last()
                    .ok_or(ParsingError::ConvertSourcePathToUseTreeNotEnoughSegmentsError)?;
                let use_tree = UseTree::Name(UseName {
                    ident: last_segment.to_owned(),
                });
                (use_tree, &segments[..segments.len() - 1])
            }
            SourcePath::Glob(ref segments) => {
                let use_tree = UseTree::Glob(UseGlob {
                    star_token: Default::default(),
                });
                (use_tree, segments.as_slice())
            }
            SourcePath::Rename(ref segments, rename) => {
                let last_segment = segments
                    .last()
                    .ok_or(ParsingError::ConvertSourcePathToUseTreeNotEnoughSegmentsError)?;
                let use_tree = UseTree::Rename(UseRename {
                    ident: last_segment.to_owned(),
                    as_token: Default::default(),
                    rename: rename.to_owned(),
                });
                (use_tree, &segments[..segments.len() - 1])
            }
            SourcePath::Group => return Err(ParsingError::ConvertSourcePathGroupToUseTreeError),
        };
        if segments.is_empty() {
            return Err(ParsingError::ConvertSourcePathToUseTreeNotEnoughSegmentsError);
        }
        for segment in segments.iter().rev().skip(1) {
            let use_path = UsePath {
                ident: segment.to_owned(),
                colon2_token: Default::default(),
                tree: Box::new(use_tree),
            };
            use_tree = UseTree::Path(use_path);
        }
        Ok(use_tree)
    }
}

impl TryFrom<SourcePath> for Path {
    type Error = ParsingError;

    fn try_from(value: SourcePath) -> Result<Self, Self::Error> {
        match value {
            SourcePath::Name(segments) => {
                let path = Path {
                    leading_colon: None,
                    segments: segments
                        .iter()
                        .map(|ident| syn::PathSegment {
                            ident: ident.to_owned(),
                            arguments: Default::default(),
                        })
                        .collect(),
                };
                Ok(path)
            }
            _ => Err(ParsingError::ConvertSourcePathToPathError),
        }
    }
}

pub trait UseTreeExtras {
    fn get_use_items_of_use_group(&self) -> Vec<UseTree>;
    fn is_use_tree_root_path_keyword(&self) -> bool;
    fn is_use_tree_root_crate_keyword(&self) -> bool;
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

    fn is_use_tree_root_path_keyword(&self) -> bool {
        if let SourcePath::Name(segments) = self.into() {
            return segments[0] == "crate" || segments[0] == "super" || segments[0] == "self";
        }
        false
    }

    fn is_use_tree_root_crate_keyword(&self) -> bool {
        if let SourcePath::Name(segments) = self.into() {
            return segments[0] == "crate";
        }
        false
    }
}

pub trait ItemExtras {
    fn get_use_items_of_use_group(&self) -> Vec<Item>;
    fn get_item_use(&self) -> Option<&ItemUse>;
    fn extract_visibility(&self) -> Option<&Visibility>;
    fn replace_glob_with_name_ident(self, ident: Ident) -> Option<Item>;
}

impl ItemExtras for Item {
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

impl Sortable for Item {
    fn sort(&self, other: &Self) -> std::cmp::Ordering {
        let self_name = ItemName::from(self).get_name();
        let other_name = ItemName::from(other).get_name();
        match (self_name, other_name) {
            (Some(self_name), Some(other_name)) => self_name.cmp(&other_name),
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (None, None) => std::cmp::Ordering::Equal,
        }
    }
}

// extract item name from syn::Item, syn::ImplItem, syn::TraitItem

use super::SourcePath;

use quote::ToTokens;
use std::fmt::Display;
use syn::{Ident, ImplItem, Item, ItemUse, TraitItem, Type};

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
            Self::TypeStringAndIdent(ts, i) => write!(f, "{} ({ts})", i),
            Self::TypeStringAndRenamed(ts, i, r) => {
                write!(f, "{} as {} ({ts})", i, r)
            }
            Self::TypeStringAndNameString(ts, ns) => write!(f, "{ns} ({ts})"),
            Self::TypeStringAndTraitAndNameString(ts, t, ns) => {
                write!(f, "{} for {ns} ({ts})", t)
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
}

impl From<&ItemUse> for ItemName {
    fn from(item_use: &ItemUse) -> Self {
        match item_use.into() {
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
                    SourcePath::from(trait_).get_last().map(|i| i.to_owned())
                } else {
                    None
                };
                match item_impl.self_ty.as_ref() {
                    // at current state of code, we only support Path and Reference
                    Type::Path(type_path) => {
                        let path_target = match SourcePath::from(type_path).get_last() {
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
                            let path_target = match SourcePath::from(type_path).get_last() {
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
                                item_impl.self_ty.to_token_stream().to_string(),
                            )
                        } else {
                            ItemName::TypeStringAndNameString(
                                "Impl".into(),
                                item_impl.self_ty.to_token_stream().to_string(),
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
            Item::Use(item_use) => item_use.into(),
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
                match SourcePath::from(impl_item_macro).get_last() {
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

impl From<&TraitItem> for ItemName {
    fn from(trait_item: &TraitItem) -> Self {
        match trait_item {
            TraitItem::Const(trait_item_const) => ItemName::TypeStringAndIdent(
                "Trait Const".into(),
                trait_item_const.ident.to_owned(),
            ),
            TraitItem::Fn(trait_item_fn) => {
                ItemName::TypeStringAndIdent("Trait Fn".into(), trait_item_fn.sig.ident.to_owned())
            }
            TraitItem::Macro(trait_item_macro) => {
                match SourcePath::from(trait_item_macro).get_last() {
                    Some(ident) => {
                        ItemName::TypeStringAndIdent("Trait Macro".into(), ident.to_owned())
                    }
                    None => ItemName::TypeString("Trait Macro".into()),
                }
            }
            TraitItem::Type(trait_item_type) => {
                ItemName::TypeStringAndIdent("Trait Type".into(), trait_item_type.ident.to_owned())
            }
            TraitItem::Verbatim(_) => ItemName::TypeString("Trait Verbatim".into()),
            _ => ItemName::None,
        }
    }
}

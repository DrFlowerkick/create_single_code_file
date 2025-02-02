// functions to minimize use and path statements. This will although remove the crate keyword from use and path statements.
// Since cg-fusion fuses all crates into one, the crate keyword may lead to unexpected behavior.
// Therefore, this function removes the crate keyword from use and path statements while minimizing the path.

use super::{ProcessingImplBlocksState, ProcessingResult};
use crate::{
    add_context,
    challenge_tree::{NodeType, PathElement, SourcePathWalker},
    configuration::CgCli,
    parsing::SourcePath,
    CgData,
};

use anyhow::{anyhow, Context};
use petgraph::stable_graph::NodeIndex;
use proc_macro2::Span;
use syn::{fold::Fold, Ident, Item, Path, PathSegment, UseTree};

pub struct ProcessingCrateUseAndPathState;

impl<O: CgCli> CgData<O, ProcessingCrateUseAndPathState> {
    pub fn path_minimizing_of_use_and_path_statements(
        mut self,
    ) -> ProcessingResult<CgData<O, ProcessingImplBlocksState>> {
        // 1. minimize use statements
        let use_item_indices: Vec<(NodeIndex, SourcePath)> = self
            .iter_crates()
            .flat_map(|(n, _, _)| self.iter_syn_items(n))
            .filter_map(|(n, i)| {
                if let syn::Item::Use(use_item) = i {
                    Some((n, SourcePath::from(use_item)))
                } else {
                    None
                }
            })
            .collect();
        for (use_item_index, use_item_path) in use_item_indices {
            let new_use_item_path =
                self.resolving_crate_source_path(use_item_index, use_item_path)?;
            let new_use_item_tree: UseTree = new_use_item_path.try_into()?;
            if let Some(NodeType::SynItem(Item::Use(use_item))) =
                self.tree.node_weight_mut(use_item_index)
            {
                use_item.tree = new_use_item_tree;
            }
        }

        // 2. minimize path statements, removing crate keyword from path statements"
        let all_syn_items: Vec<NodeIndex> = self
            .iter_crates()
            .flat_map(|(n, _, _)| {
                self.iter_syn(n).filter_map(|(n, i)| {
                    if let NodeType::SynItem(Item::Mod(_)) = i {
                        // filter modules
                        None
                    } else {
                        Some(n)
                    }
                })
            })
            .collect();
        for syn_index in all_syn_items {
            if let Some(cloned_item) = self.clone_syn_item(syn_index) {
                let mut folder = CratePathFolder {
                    graph: &self,
                    node: syn_index,
                };
                let new_item = match cloned_item {
                    Item::Impl(mut impl_item) => {
                        // only fold trait_ and self_ty of impl_item
                        if let Some((pre_token, trait_path, post_token)) = impl_item.trait_ {
                            let trait_path = folder.fold_path(trait_path);
                            impl_item.trait_ = Some((pre_token, trait_path, post_token));
                        }
                        impl_item.self_ty =
                            Box::new(folder.fold_type(impl_item.self_ty.as_ref().to_owned()));
                        Item::Impl(impl_item)
                    }
                    Item::Trait(_) => {
                        // do not fold trait items directly
                        cloned_item
                    }
                    _ => folder.fold_item(cloned_item),
                };

                if let Some(NodeType::SynItem(item)) = self.tree.node_weight_mut(syn_index) {
                    *item = new_item;
                }
            }
            if let Some(cloned_impl_item) = self.clone_syn_impl_item(syn_index) {
                let mut folder = CratePathFolder {
                    graph: &self,
                    node: syn_index,
                };
                let new_impl_item = folder.fold_impl_item(cloned_impl_item);
                if let Some(NodeType::SynImplItem(impl_item)) = self.tree.node_weight_mut(syn_index)
                {
                    *impl_item = new_impl_item;
                }
            }
            if let Some(cloned_trait_item) = self.clone_syn_trait_item(syn_index) {
                let mut folder = CratePathFolder {
                    graph: &self,
                    node: syn_index,
                };
                let new_trait_item = folder.fold_trait_item(cloned_trait_item);
                if let Some(NodeType::SynTraitItem(trait_item)) =
                    self.tree.node_weight_mut(syn_index)
                {
                    *trait_item = new_trait_item;
                }
            }
        }

        Ok(self.set_state(ProcessingImplBlocksState))
    }

    pub(crate) fn resolving_crate_source_path(
        &self,
        path_item_index: NodeIndex,
        source_path: SourcePath,
    ) -> ProcessingResult<SourcePath> {
        // get path properties
        let (segments, glob, rename) = match &source_path {
            SourcePath::Group => {
                unreachable!("use groups have been expanded before.");
            }
            // glob is still possible, if it points to external crate
            SourcePath::Glob(ref segments) => (segments, true, None),
            SourcePath::Name(ref segments) => (segments, false, None),
            SourcePath::Rename(ref segments, ref renamed) => {
                (segments, false, Some(renamed.to_owned()))
            }
        };
        let mut remaining_external_segments: Option<Vec<Ident>> = None;
        let mut path_leaf: Option<NodeIndex> = None;
        let mut path_walker = SourcePathWalker::new(source_path.clone(), path_item_index);
        while let Some(path_element) = path_walker.next(self) {
            match path_element {
                PathElement::PathCouldNotBeParsed => return Ok(source_path),
                PathElement::Group => {
                    unreachable!("Use groups have been expanded before.");
                }
                PathElement::Glob(_) => {
                    unreachable!("Local use globs have been expanded before. Only external globs are possible, which will return \
                                  PathElement::ExternalPackage before reaching glob.");
                }
                PathElement::ExternalPackage => {
                    if let Some(leaf_index) = path_leaf {
                        // This is only possible, if a path element points toward a use statement,
                        // which imports external code. Minimize path to this use statement and
                        // append remaining segments of external use statement
                        if let Some(external_use_ident) = self.get_ident(leaf_index) {
                            if let Some(pos) =
                                segments.iter().position(|s| *s == external_use_ident)
                            {
                                remaining_external_segments = Some(Vec::from(&segments[pos + 1..]));
                                break;
                            }
                        }
                        return Ok(source_path);
                    } else {
                        // path directly starts with external package
                        return Ok(source_path);
                    }
                }
                PathElement::Item(item_index) | PathElement::ItemRenamed(item_index, _) => {
                    // collect item index, rename is already extracted from SourcePath
                    path_leaf = Some(item_index);
                    //segments_slice = &segments_slice[1..];
                }
            }
        }
        // compare crates of active path leaf and path_item_index
        let path_leaf = path_leaf.context(add_context!("Expected index of path leaf."))?;
        let path_leaf_nodes = self.get_crate_path_nodes(path_leaf);
        let path_item_nodes = self.get_crate_path_nodes(path_item_index);
        let mut new_path: Vec<Ident> = if path_leaf_nodes[0] != path_item_nodes[0] {
            // return path of leaf starting from it's crate
            path_leaf_nodes
                .iter()
                .map(|n| {
                    self.get_ident(*n)
                        .ok_or(anyhow!("{}", add_context!("Expected ident of path node.")).into())
                })
                .collect::<ProcessingResult<Vec<_>>>()?
        } else {
            // identify best path inside crate from path_item_index to path_leaf
            let pos_junction = path_item_nodes
                .iter()
                .zip(path_leaf_nodes.iter())
                .take_while(|(a, b)| a == b)
                .count();
            let (from_junction_leaf_ident, num_super) = if pos_junction == path_leaf_nodes.len() {
                // path_item is deeper in tree than path_leaf
                let leaf_ident = self
                    .get_ident(path_leaf_nodes[pos_junction - 1])
                    .context(add_context!("Expected ident of path node."))?;
                let num_super = path_item_nodes.len() - pos_junction;
                (vec![leaf_ident], num_super)
            } else {
                // path leaf is deeper in tree than path_item
                let from_junction_leaf_ident = path_leaf_nodes[pos_junction..]
                    .iter()
                    .map(|n| {
                        self.get_ident(*n).ok_or(
                            anyhow!("{}", add_context!("Expected ident of path node.")).into(),
                        )
                    })
                    .collect::<ProcessingResult<Vec<_>>>()?;
                let num_super = path_item_nodes.len() - pos_junction - 1;
                (from_junction_leaf_ident, num_super)
            };
            let mut new_path = vec![Ident::new("super", Span::call_site()); num_super];
            new_path.extend(from_junction_leaf_ident);
            new_path
        };

        if let Some(res) = remaining_external_segments.take() {
            new_path.extend(res);
        }

        let new_path = match (glob, rename) {
            (true, None) => SourcePath::Glob(new_path),
            (false, Some(renamed)) => SourcePath::Rename(new_path, renamed),
            (false, None) => SourcePath::Name(new_path),
            _ => unreachable!(),
        };
        Ok(new_path)
    }
}

pub struct CratePathFolder<'a, O: CgCli> {
    graph: &'a CgData<O, ProcessingCrateUseAndPathState>,
    node: NodeIndex,
}

impl<O: CgCli> Fold for CratePathFolder<'_, O> {
    fn fold_path(&mut self, path: Path) -> Path {
        let source_path = SourcePath::from(&path);
        let resolved_path = self
            .graph
            .resolving_crate_source_path(self.node, source_path)
            .expect("resolving crate source path failed");
        let resolved_path: Path = resolved_path
            .try_into()
            .expect("resolving crate source path failed");
        // rebuild arguments of segments from input path
        let resolved_path = Path {
            leading_colon: path.leading_colon,
            segments: resolved_path
                .segments
                .iter()
                .map(|s| {
                    if let Some(arguments) = path
                        .segments
                        .iter()
                        .find_map(|p| (p.ident == s.ident).then_some(p.arguments.to_owned()))
                    {
                        PathSegment {
                            ident: s.ident.to_owned(),
                            arguments,
                        }
                    } else {
                        s.to_owned()
                    }
                })
                .collect(),
        };
        resolved_path
    }
}

#[cfg(test)]
mod tests {

    use crate::parsing::ItemName;
    use quote::ToTokens;
    use syn::Item;

    use super::super::tests::setup_processing_test;

    #[test]
    fn test_path_minimizing_of_use_and_path_statements() {
        // preparation
        let cg_data = setup_processing_test(false)
            .add_challenge_dependencies()
            .unwrap()
            .add_src_files()
            .unwrap()
            .expand_use_statements()
            .unwrap()
            // action to test
            .path_minimizing_of_use_and_path_statements()
            .unwrap();

        // validation
        let (cg_fusion_binary_test_lib_index, ..) = cg_data
            .iter_crates()
            .find(|(_, _, src_file)| src_file.name == "cg_fusion_binary_test")
            .unwrap();
        let use_action_index = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_lib_index)
            .find_map(|(n, i)| {
                if let Some(ident) = ItemName::from(i).get_ident_in_name_space() {
                    if ident == "Action" {
                        Some(n)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap();
        let use_action = cg_data
            .get_syn_item(use_action_index)
            .unwrap()
            .to_token_stream()
            .to_string();
        assert_eq!(use_action, "use action :: Action ;");

        let (my_map_two_dim_index, ..) = cg_data
            .iter_crates()
            .find(|(_, _, src_file)| src_file.name == "my_map_two_dim")
            .unwrap();
        let (my_map_point_index, _) = cg_data
            .iter_syn_item_neighbors(my_map_two_dim_index)
            .find(|(_, i)| {
                if let Some(ident) = ItemName::from(*i).get_ident_in_name_space() {
                    ident == "my_map_point"
                } else {
                    false
                }
            })
            .unwrap();
        let use_compass_index = cg_data
            .iter_syn_item_neighbors(my_map_point_index)
            .find_map(|(n, i)| {
                if let Some(ident) = ItemName::from(i).get_ident_in_name_space() {
                    if ident == "Compass" {
                        Some(n)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap();
        let use_compass = cg_data
            .get_syn_item(use_compass_index)
            .unwrap()
            .to_token_stream()
            .to_string();
        assert_eq!(use_compass, "use my_compass :: Compass ;");

        let (_, impl_default_block_of_go) = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_lib_index)
            .find(|(_, i)| {
                let name = ItemName::from(*i);
                if let ItemName::TypeStringAndTraitAndNameString(_, trait_ident, _) = name {
                    trait_ident == "Default"
                } else {
                    false
                }
            })
            .unwrap();
        let Item::Impl(impl_default_block_of_go) = impl_default_block_of_go else {
            panic!("Expected impl block of Go.");
        };
        let impl_default_block_of_go_impl_reference = impl_default_block_of_go
            .self_ty
            .to_token_stream()
            .to_string();
        assert_eq!(impl_default_block_of_go_impl_reference, "Go");

        let mod_action_index = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_lib_index)
            .find_map(|(n, i)| {
                if let Item::Mod(item_mod) = i {
                    (item_mod.ident == "action").then_some(n)
                } else {
                    None
                }
            })
            .unwrap();

        let use_fmt_index = cg_data
            .iter_syn_item_neighbors(mod_action_index)
            .find_map(|(n, i)| {
                if let Item::Use(use_item) = i {
                    if let Some(ident) = ItemName::from(use_item).get_ident_in_name_space() {
                        (ident == "fmt").then_some(n)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap();
        let use_fmt = cg_data
            .get_syn_item(use_fmt_index)
            .unwrap()
            .to_token_stream()
            .to_string();
        assert_eq!(use_fmt, "use super :: fmt ;");

        let use_fmt_display_index = cg_data
            .iter_syn_item_neighbors(mod_action_index)
            .find_map(|(n, i)| {
                if let Item::Use(use_item) = i {
                    if let Some(ident) = ItemName::from(use_item).get_ident_in_name_space() {
                        (ident == "Display").then_some(n)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap();
        let use_fmt_display = cg_data
            .get_syn_item(use_fmt_display_index)
            .unwrap()
            .to_token_stream()
            .to_string();
        assert_eq!(use_fmt_display, "use super :: fmt :: Display ;");
    }
}

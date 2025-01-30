// functions to walk the challenge tree

use petgraph::{
    stable_graph::NodeIndex,
    visit::{Bfs, EdgeRef},
};
// petgraph uses FixedBitSet as VisitMap for Bfs
use fixedbitset::FixedBitSet;
use syn::{Ident, Item};

use crate::{
    parsing::{ItemName, SourcePath},
    CgData,
};

use super::{ChallengeTree, EdgeType, NodeType, VariableReferences};

pub trait BfsWalker {
    fn next(&mut self, graph: &ChallengeTree) -> Option<NodeIndex>;
    fn stack_len(&self) -> usize;
}

pub struct BfsByEdgeType {
    walker: Bfs<NodeIndex, FixedBitSet>,
    edge_type: EdgeType,
}

impl BfsByEdgeType {
    pub fn new(graph: &ChallengeTree, start: NodeIndex, edge_type: EdgeType) -> Self {
        Self {
            walker: Bfs::new(graph, start),
            edge_type,
        }
    }

    pub fn into_iter(self, graph: &ChallengeTree) -> BfsIterator<'_, BfsByEdgeType> {
        BfsIterator {
            walker: self,
            graph,
        }
    }
}

impl BfsWalker for BfsByEdgeType {
    // code adapted from petgraph, see Bfs implementation of next()
    fn next(&mut self, graph: &ChallengeTree) -> Option<NodeIndex> {
        if let Some(node) = self.walker.stack.pop_front() {
            for successor in graph
                .edges(node)
                .filter(|e| *e.weight() == self.edge_type)
                .map(|e| e.target())
            {
                // see trait VisitMap of petgraph for visit()
                // return true, if first time visited
                if !self.walker.discovered.put(successor.index()) {
                    self.walker.stack.push_back(successor);
                }
            }

            return Some(node);
        }
        None
    }

    fn stack_len(&self) -> usize {
        self.walker.stack.len()
    }
}

pub struct BfsIterator<'a, T: BfsWalker> {
    walker: T,
    graph: &'a ChallengeTree,
}

impl<T: BfsWalker> Iterator for BfsIterator<'_, T> {
    type Item = NodeIndex;

    fn next(&mut self) -> Option<Self::Item> {
        self.walker.next(self.graph)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.walker.stack_len(), Some(self.graph.node_count()))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PathElement {
    ExternalPackage,
    Group,
    Glob(NodeIndex),
    Item(NodeIndex),
    ItemRenamed(NodeIndex, Ident),
    PathCouldNotBeParsed,
}

#[derive(Debug)]
pub struct SourcePathWalker {
    source_path: SourcePath,
    current_node_index: NodeIndex,
    current_index: usize,
    variables: VariableReferences,
    walker_finished: bool,
}

impl SourcePathWalker {
    pub fn new(source_path: SourcePath, path_item_index: NodeIndex) -> Self {
        Self {
            source_path,
            current_node_index: path_item_index,
            current_index: 0,
            variables: VariableReferences::default(),
            walker_finished: false,
        }
    }

    pub fn with_variables(
        source_path: SourcePath,
        path_item_index: NodeIndex,
        variables: VariableReferences,
    ) -> Self {
        Self {
            source_path,
            current_node_index: path_item_index,
            current_index: 0,
            variables,
            walker_finished: false,
        }
    }

    fn set_current_node_to_module_of_it<O, S>(&mut self, graph: &CgData<O, S>) -> bool {
        if let Some(crate_index) = graph.get_syn_module_index(self.current_node_index) {
            self.current_node_index = crate_index;
            false
        } else {
            self.walker_finished = true;
            true
        }
    }

    pub fn next<O, S>(&mut self, graph: &CgData<O, S>) -> Option<PathElement> {
        if self.walker_finished {
            return None;
        }
        let (segments, glob, rename) = match self.source_path {
            SourcePath::Group => {
                self.walker_finished = true;
                return Some(PathElement::Group);
            }
            SourcePath::Glob(ref segments) => (segments, true, None),
            SourcePath::Name(ref segments) => (segments, false, None),
            SourcePath::Rename(ref segments, ref renamed) => {
                (segments, false, Some(renamed.to_owned()))
            }
        };
        if self.current_index == segments.len() {
            // if last segment of glob points toward reimported module, we now return the index of this module
            assert!(glob);
            self.walker_finished = true;
            return Some(PathElement::Glob(self.current_node_index));
        }
        let segment = segments[self.current_index].to_owned();
        let is_first = self.current_index == 0;
        let is_last = self.current_index == segments.len() - 1;
        self.walker_finished = is_last && !glob;
        self.current_index += 1;
        match segment.to_string().as_str() {
            "Self" => {
                // referencing Self of impl fn or in trait fn
                if graph.is_syn_impl_item(self.current_node_index) {
                    if let Some(self_type_index) =
                        graph.get_syn_impl_item_self_type_node(self.current_node_index)
                    {
                        self.current_node_index = self_type_index;
                        return Some(PathElement::Item(self.current_node_index));
                    }
                    self.walker_finished = true;
                    return Some(PathElement::PathCouldNotBeParsed);
                } else {
                    // In trait fn it is not possible to identify Self. This is only possible,
                    // if trait is impl by a type
                    self.walker_finished = true;
                    return Some(PathElement::PathCouldNotBeParsed);
                }
            }
            "crate" => {
                // module of current crate
                self.current_node_index =
                    if let Some(crate_index) = graph.get_crate_index(self.current_node_index) {
                        crate_index
                    } else {
                        self.walker_finished = true;
                        return Some(PathElement::PathCouldNotBeParsed);
                    };
                return Some(PathElement::Item(self.current_node_index));
            }
            "self" => {
                // set current node to module of item
                if self.set_current_node_to_module_of_it(graph) {
                    return Some(PathElement::PathCouldNotBeParsed);
                };
                return Some(PathElement::Item(self.current_node_index));
            }
            "super" => {
                // super module
                if is_first {
                    // if is first, set current node to module of item
                    if self.set_current_node_to_module_of_it(graph) {
                        return Some(PathElement::PathCouldNotBeParsed);
                    };
                }
                // get module of current node
                if self.set_current_node_to_module_of_it(graph) {
                    return Some(PathElement::PathCouldNotBeParsed);
                };
                return Some(PathElement::Item(self.current_node_index));
            }
            _ => {
                if is_first {
                    // check if path starts with a variable name
                    if let Some(variable_node) = self.variables.get_node_index(&segment) {
                        self.current_node_index = variable_node;
                        return Some(PathElement::Item(self.current_node_index));
                    }
                    // check if path starts with external dependency
                    if graph
                        .iter_external_dependencies()
                        .any(|dep_name| segment == dep_name)
                    {
                        self.walker_finished = true;
                        return Some(PathElement::ExternalPackage);
                    }
                    // check if path starts with local package dependency
                    if let Some((local_package_index, _)) =
                        graph.iter_lib_crates().find(|(_, cf)| segment == cf.name)
                    {
                        self.current_node_index = local_package_index;
                        return Some(PathElement::Item(self.current_node_index));
                    }
                    // if none of above set current node to module of item
                    if self.set_current_node_to_module_of_it(graph) {
                        return Some(PathElement::PathCouldNotBeParsed);
                    };
                }
                // if node of current_node_index is a struct, enum, or union AND if
                // one or more impl for this node exists, search items of these impl
                let next_item = if matches!(
                    graph.get_syn_item(self.current_node_index),
                    Some(Item::Enum(_)) | Some(Item::Struct(_)) | Some(Item::Union(_))
                ) {
                    // iter all impl items of all impl blocks linked to current_node_index
                    graph
                        .iter_impl_blocks_of_item(self.current_node_index)
                        .flat_map(|(n, _)| {
                            graph
                                .iter_syn_neighbors(n)
                                .filter_map(|(n_impl, nt)| match nt {
                                    NodeType::SynImplItem(impl_item) => ItemName::from(impl_item)
                                        .get_ident_in_name_space()
                                        .map(|id| (n_impl, nt, id)),
                                    _ => None,
                                })
                        })
                        .find(|(_, _, id)| *id == segment)
                } else {
                    // iter all syn neighbors, which can although be trait items
                    graph
                        .iter_syn_neighbors(self.current_node_index)
                        .filter_map(|(n, nt)| match nt {
                            NodeType::SynItem(item) => ItemName::from(item)
                                .get_ident_in_name_space()
                                .map(|id| (n, nt, id)),
                            NodeType::SynTraitItem(trait_item) => ItemName::from(trait_item)
                                .get_ident_in_name_space()
                                .map(|id| (n, nt, id)),
                            _ => None,
                        })
                        .find(|(_, _, id)| *id == segment)
                };

                if let Some((item_index, node_type, _)) = next_item {
                    match node_type {
                        // we need mod, use, struct, enum, union, all other syn items which have an ident, all syn impl items, which have an ident.
                        NodeType::SynItem(Item::Mod(_)) => {
                            // found local module
                            self.current_node_index = item_index;
                            if is_last && glob {
                                self.walker_finished = true;
                                return Some(PathElement::Glob(self.current_node_index));
                            }
                            return Some(PathElement::Item(self.current_node_index));
                        }
                        NodeType::SynItem(Item::Use(item_use)) => {
                            // found reimported item -> get index of it
                            if let Ok(path_element) =
                                graph.get_path_leaf(item_index, &item_use.tree)
                            {
                                match path_element {
                                    PathElement::ExternalPackage => {
                                        return Some(PathElement::ExternalPackage)
                                    }
                                    PathElement::Glob(_) | PathElement::Group => {
                                        unreachable!(
                                            "filter_map use statements which end on name or rename"
                                        );
                                    }
                                    PathElement::Item(path_item_index)
                                    | PathElement::ItemRenamed(path_item_index, _) => {
                                        let result = Some(PathElement::Item(item_index));
                                        self.current_node_index = path_item_index;
                                        return result;
                                    }
                                    PathElement::PathCouldNotBeParsed => {
                                        // could not find module of use statement
                                        return Some(PathElement::PathCouldNotBeParsed);
                                    }
                                }
                            }
                        }
                        NodeType::SynItem(_) => {
                            self.current_node_index = item_index;
                            if is_last {
                                if let Some(renamed) = rename {
                                    return Some(PathElement::ItemRenamed(
                                        self.current_node_index,
                                        renamed.to_owned(),
                                    ));
                                }
                            }
                            return Some(PathElement::Item(self.current_node_index));
                        }
                        NodeType::SynImplItem(_) | NodeType::SynTraitItem(_) => {
                            self.current_node_index = item_index;
                            return Some(PathElement::Item(self.current_node_index));
                        }
                        _ => unreachable!("Filtering for SynItem, SynImplItem, and SynTraitItem."),
                    }
                }
            }
        }
        self.walker_finished = true;
        Some(PathElement::PathCouldNotBeParsed)
    }

    pub fn into_iter<O, S>(self, graph: &CgData<O, S>) -> SourcePathIterator<'_, O, S> {
        SourcePathIterator {
            walker: self,
            graph,
        }
    }
}

pub struct SourcePathIterator<'a, O, S> {
    walker: SourcePathWalker,
    graph: &'a CgData<O, S>,
}

impl<O, S> Iterator for SourcePathIterator<'_, O, S> {
    type Item = PathElement;

    fn next(&mut self) -> Option<Self::Item> {
        self.walker.next(self.graph)
    }
}

#[cfg(test)]
mod tests {

    use crate::parsing::PathAnalysis;

    use super::super::super::processing::tests::setup_processing_test;
    use super::*;

    use syn::UseTree;

    #[test]
    fn test_source_path_walker() {
        // preparation
        let cg_data = setup_processing_test()
            .add_challenge_dependencies()
            .unwrap()
            .add_src_files()
            .unwrap();

        // test case 1: test use statements of challenge
        let (challenge_bin_crate_index, _) = cg_data.get_challenge_bin_crate().unwrap();
        let use_statements = cg_data
            .iter_syn_item_neighbors(challenge_bin_crate_index)
            .filter(|(_, i)| if let Item::Use(_) = i { true } else { false })
            .collect::<Vec<_>>();
        let use_statement_targets = use_statements
            .iter()
            .filter_map(|(n, i)| match i {
                Item::Use(item_use) => {
                    let source_path = item_use.tree.extract_path();
                    let walker = SourcePathWalker::new(source_path, *n);
                    walker.into_iter(&cg_data).last()
                }
                _ => unreachable!("use statement expected"),
            })
            .collect::<Vec<_>>();
        assert!(matches!(use_statement_targets[0], PathElement::Glob(_)));
        assert!(matches!(use_statement_targets[1], PathElement::Group));
        assert!(matches!(use_statement_targets[2], PathElement::Item(_)));

        // test case 2: test use statements of my_map_two_dim
        let (my_map_two_dim_mod_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "my_map_two_dim")
            .unwrap();
        let (my_map_point_mod_index, _) = cg_data
            .iter_syn_items(my_map_two_dim_mod_index)
            .filter_map(|(n, i)| {
                if let Item::Mod(_) = i {
                    ItemName::from(i)
                        .get_ident_in_name_space()
                        .map(|id| (n, id))
                } else {
                    None
                }
            })
            .find(|(_, c)| c == "my_map_point")
            .unwrap();
        let use_of_my_map_point: Vec<(NodeIndex, Ident, &UseTree)> = cg_data
            .iter_syn_item_neighbors(my_map_point_mod_index)
            .filter_map(|(n, i)| {
                if let Item::Use(item_use) = i {
                    match item_use.tree.extract_path() {
                        SourcePath::Name(segments) | SourcePath::Glob(segments) => {
                            Some((n, segments.last().unwrap().to_owned(), &item_use.tree))
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect();
        let (use_glob_index_my_compass, _, use_glob_tree_my_compass) = use_of_my_map_point
            .iter()
            .find(|(_, id, _)| id == "my_compass")
            .unwrap();
        let my_compass_mod_index = cg_data
            .iter_syn_item_neighbors(my_map_point_mod_index)
            .filter_map(|(n, i)| {
                if let Item::Mod(_) = i {
                    ItemName::from(i)
                        .get_ident_in_name_space()
                        .map(|id| (n, id))
                } else {
                    None
                }
            })
            .find(|(_, id)| id == "my_compass")
            .unwrap()
            .0;
        let path_elements_of_use_glob_my_compass: Vec<PathElement> = SourcePathWalker::new(
            use_glob_tree_my_compass.extract_path(),
            *use_glob_index_my_compass,
        )
        .into_iter(&cg_data)
        .collect();
        assert_eq!(
            *path_elements_of_use_glob_my_compass.iter().last().unwrap(),
            PathElement::Glob(my_compass_mod_index)
        );

        // test case 3: test use statements of cg_fusion_binary_test
        let (cg_fusion_binary_test_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "cg_fusion_binary_test")
            .unwrap();
        let use_of_cg_fusion_binary_test: Vec<(NodeIndex, Ident, &UseTree)> = cg_data
            .iter_syn_item_neighbors(cg_fusion_binary_test_index)
            .filter_map(|(n, i)| {
                if let Item::Use(item_use) = i {
                    match item_use.tree.extract_path() {
                        SourcePath::Name(segments) | SourcePath::Glob(segments) => {
                            Some((n, segments.last().unwrap().to_owned(), &item_use.tree))
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect();
        let (use_glob_index_my_map_two_dim, _, use_glob_tree_my_map_two_dim) =
            use_of_cg_fusion_binary_test
                .iter()
                .find(|(_, id, _)| id == "my_map_two_dim")
                .unwrap();
        let (my_map_two_dim_index, _) = cg_data
            .iter_lib_crates()
            .find(|(_, c)| c.name == "my_map_two_dim")
            .unwrap();
        let path_elements_of_use_glob_my_map_two_dim: Vec<PathElement> = SourcePathWalker::new(
            use_glob_tree_my_map_two_dim.extract_path(),
            *use_glob_index_my_map_two_dim,
        )
        .into_iter(&cg_data)
        .collect();
        assert_eq!(
            *path_elements_of_use_glob_my_map_two_dim
                .iter()
                .last()
                .unwrap(),
            PathElement::Glob(my_map_two_dim_index)
        );
        assert_eq!(path_elements_of_use_glob_my_map_two_dim.len(), 3);
        if let PathElement::Item(cg_fusion_lib_test_index) =
            path_elements_of_use_glob_my_map_two_dim[0]
        {
            assert_eq!(
                cg_data
                    .get_verbose_name_of_tree_node(cg_fusion_lib_test_index)
                    .unwrap(),
                "cg_fusion_lib_test (library crate)"
            );
        }
        if let PathElement::Item(cg_fusion_lib_test_index) =
            path_elements_of_use_glob_my_map_two_dim[1]
        {
            assert_eq!(
                cg_data
                    .get_verbose_name_of_tree_node(cg_fusion_lib_test_index)
                    .unwrap(),
                "my_map_two_dim (Use)"
            );
        }
        if let PathElement::Item(cg_fusion_lib_test_index) =
            path_elements_of_use_glob_my_map_two_dim[2]
        {
            assert_eq!(
                cg_data
                    .get_verbose_name_of_tree_node(cg_fusion_lib_test_index)
                    .unwrap(),
                "my_map_two_dim (library crate)"
            );
        }
    }
}

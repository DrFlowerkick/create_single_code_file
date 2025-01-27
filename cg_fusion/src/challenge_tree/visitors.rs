// functions to visit the challenge tree items

use crate::{
    add_context,
    parsing::{ItemName, PathAnalysis, SourcePath},
    CgData,
};
use anyhow::{anyhow, Result};
use petgraph::stable_graph::NodeIndex;
use std::collections::HashSet;
use syn::{
    visit::Visit, Block, Expr, ExprMethodCall, FnArg, Ident, Item, LocalInit, Pat, PatIdent, Path,
    Signature, Stmt, Type, TypePath,
};

use super::{EdgeType, NodeType, PathElement, SourcePathWalker};

#[derive(Debug, Default, Clone)]
pub struct VariableReferences {
    variables: Vec<(Ident, NodeIndex)>,
}

impl VariableReferences {
    pub fn push_variable(&mut self, name: Ident, node: NodeIndex) {
        self.variables.push((name, node));
    }
    pub fn pop_variables(&mut self, number_to_pop: usize) {
        for _ in 0..number_to_pop {
            if self.variables.pop().is_none() {
                break;
            }
        }
    }
    pub fn get_node_index(&self, name: &Ident) -> Option<NodeIndex> {
        // we search backwards, because new variables are pushed to the end.
        // Since rust allows "overwriting" variable names, we have to check
        // from newest to oldest variable
        self.variables
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, n)| *n)
    }
}

// struct to collect
// - syn::Path items as SourcePath
// - ident of method calls from self
// - variable definitions, which point to user defined types
//   (ident: name of variable, SourcePath: path to user defined type)
// ToDo: try to extend to analyze local items

pub struct SynReferenceMapper<'a, O, S> {
    graph: &'a CgData<O, S>,
    node: NodeIndex,
    pub variables: VariableReferences,
    pub referenced_nodes: HashSet<NodeIndex>,
    pub leaf_nodes: HashSet<NodeIndex>,
}

impl<'a, O, S> SynReferenceMapper<'a, O, S> {
    pub fn new(graph: &'a CgData<O, S>, node: NodeIndex) -> Self {
        SynReferenceMapper {
            graph,
            node,
            variables: VariableReferences::default(),
            referenced_nodes: HashSet::new(),
            leaf_nodes: HashSet::new(),
        }
    }
    pub fn reference_use_tree_nodes(&mut self) -> Result<()> {
        if let Some(NodeType::SynItem(Item::Use(item_use))) = self.graph.tree.node_weight(self.node)
        {
            // we although add leave nodes, e.g. for impl linking
            let mut leaf: Option<NodeIndex> = None;
            // collect nodes referenced by use tree
            for path_element in
                SourcePathWalker::new(item_use.tree.extract_path(), self.node).into_iter(self.graph)
            {
                let node_reference = match path_element {
                    PathElement::Glob(_) | PathElement::Group => {
                        return Err(anyhow!(format!(
                            "{}",
                            add_context!("Expected expanded use groups and globs")
                        )))
                    }
                    PathElement::ExternalPackage | PathElement::PathCouldNotBeParsed => {
                        leaf = None;
                        break;
                    }
                    PathElement::Item(n) | PathElement::ItemRenamed(n, _) => {
                        leaf = Some(n);
                        n
                    }
                };
                self.referenced_nodes.insert(node_reference);
            }
            // add leave
            if let Some(leave_node) = leaf {
                self.leaf_nodes.insert(leave_node);
            }
        }
        Ok(())
    }
}

impl<'a, O, S> Visit<'a> for SynReferenceMapper<'a, O, S> {
    fn visit_signature(&mut self, signature: &'a Signature) {
        for argument in signature.inputs.iter() {
            let FnArg::Typed(pat_type) = argument else {
                continue;
            };
            // ident of argument
            let Pat::Ident(PatIdent { ident, .. }) = pat_type.pat.as_ref() else {
                continue;
            };
            // path of argument
            let Type::Path(TypePath { path, .. }) = pat_type.ty.as_ref() else {
                continue;
            };
            // node of argument; in syn path ItemRenamed is not possible
            let Ok(PathElement::Item(node)) = self.graph.get_path_leaf(self.node, path) else {
                continue;
            };
            self.variables.push_variable(ident.to_owned(), node);
        }

        // recursive visit
        syn::visit::visit_signature(self, signature);
    }
    fn visit_block(&mut self, block: &'a Block) {
        let mut num_variables: usize = 0;
        for statement in block.stmts.iter() {
            if let Stmt::Local(local_stmt) = statement {
                // ToDo: later this could be expanded to tuples and maybe Arrays, Vec, and Option
                if let Pat::Ident(PatIdent { ident, .. }) = &local_stmt.pat {
                    // ident of variable
                    if let Some(LocalInit { expr, .. }) = &local_stmt.init {
                        // check some expression types, which can yield type of variable
                    }
                }
            }
            // recursive visit
            syn::visit::visit_stmt(self, statement);
        }
        // remove variables collected inside of this block, because they are out of scope
        // after leaving a block
        self.variables.pop_variables(num_variables);
    }
    fn visit_path(&mut self, path: &'a Path) {
        // we although add leave nodes, e.g. for impl linking
        let mut leaf: Option<NodeIndex> = None;
        // collect nodes referenced by path
        for path_element in
            SourcePathWalker::with_variables(path.extract_path(), self.node, self.variables.clone())
                .into_iter(self.graph)
        {
            let node_reference = match path_element {
                PathElement::Glob(_) | PathElement::Group | PathElement::ItemRenamed(_, _) => {
                    unreachable!("syn path does not contain groups, globs, or rename.")
                }
                PathElement::ExternalPackage | PathElement::PathCouldNotBeParsed => {
                    leaf = None;
                    break;
                }
                PathElement::Item(n) => {
                    leaf = Some(n);
                    n
                }
            };
            self.referenced_nodes.insert(node_reference);
        }
        // add leave
        if let Some(leave_node) = leaf {
            self.leaf_nodes.insert(leave_node);
        }
        // recursive visit
        syn::visit::visit_path(self, path);
    }
    // ToDo: continue here with clean up of self_method_calls by adding node of called method to referenced_nodes
    // see expand::check_path_items_for_challenge:453
    // add function to SynReferenceMapper to add use tree to referenced_nodes and clean up expand::check_path_items_for_challenge:407
    // afterward clean up expand::check_path_items_for_challenge:430-472 and TEST
    // than continue visit_block
    fn visit_expr_method_call(&mut self, expr_method_call: &'a ExprMethodCall) {
        if let Expr::Path(expr_path) = expr_method_call.receiver.as_ref() {
            if let SourcePath::Name(segments) = expr_path.path.extract_path() {
                if segments.len() == 1 && segments[0] == "self" {
                    // add reference to method call, if receiver is self
                    for (impl_method, _) in self
                        .graph
                        .get_parent_index_by_edge_type(self.node, EdgeType::Syn)
                        .map(|n| {
                            self.graph
                                .get_parent_index_by_edge_type(n, EdgeType::Implementation)
                        })
                        .flatten()
                        .into_iter()
                        .flat_map(|n| self.graph.iter_impl_blocks_of_item(n))
                        .flat_map(|(n, _)| self.graph.iter_syn_impl_item(n))
                        .filter(|(n, _)| !self.graph.is_required_by_challenge(*n))
                        .filter(|(_, i)| {
                            if let Some(name) = ItemName::from(*i).get_ident_in_name_space() {
                                name == expr_method_call.method
                            } else {
                                false
                            }
                        })
                    {
                        // It is possible to have the same method name in different impl blocks of an item,
                        // if the item has generic parameters and impl blocks, each defining a specific
                        // type for the generic parameter. These type specific impl blocks may share
                        // method names, because they are identified by the specific type first and than
                        // by method name. Same is true if different traits with similar method names are
                        // implemented.
                        self.referenced_nodes.insert(impl_method);
                    }
                }
            }
        }
        // recursive visit
        syn::visit::visit_expr_method_call(self, expr_method_call);
    }
}

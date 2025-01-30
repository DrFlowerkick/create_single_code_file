// functions to visit the challenge tree items

use crate::{
    add_context,
    parsing::{ItemName, SourcePath},
    CgData,
};
use anyhow::{anyhow, Result};
use petgraph::stable_graph::NodeIndex;
use std::collections::HashSet;
use syn::{
    visit::Visit, Block, Expr, ExprMethodCall, FnArg, Ident, ImplItem, Item, LocalInit, Pat,
    PatIdent, Path, ReturnType, Signature, Stmt, Type, TypePath,
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

// struct to parse and collect referenced nodes and leaf nodes (nodes at the end of
// syn::Path elements) from a parsed code snippet.
// variables (ident and node of type) are collected, too. VariableReferences is used
// in SourcePathWalker to traverse Path elements, which start with variable names.

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
                SourcePathWalker::new(item_use.into(), self.node).into_iter(self.graph)
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
            let Ok(PathElement::Item(node)) = self.graph.get_path_leaf(self.node, path.into())
            else {
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
            // first recursive visit...
            syn::visit::visit_stmt(self, statement);
            // ...than try to add variables. This order prevents usage of variables before they are defined
            let Stmt::Local(local_stmt) = statement else {
                continue;
            };
            // At current state we only check single named variables (no tuples or enum constrictions
            // like let Some(var) = ...).
            // ToDo: later this could be expanded to tuples and maybe Arrays, Vec, and Option.

            match &local_stmt.pat {
                // check if type is explicitly defined
                Pat::Type(pat_type) => {
                    let Pat::Ident(PatIdent { ident, .. }) = pat_type.pat.as_ref() else {
                        continue;
                    };
                    let Type::Path(type_path) = pat_type.ty.as_ref() else {
                        continue;
                    };
                    if let Ok(PathElement::Item(node)) =
                        self.graph.get_path_leaf(self.node, type_path.into())
                    {
                        self.variables.push_variable(ident.to_owned(), node);
                        num_variables += 1;
                    }
                }
                // check if variable is given as simple ident
                Pat::Ident(PatIdent { ident, .. }) => {
                    // ident of variable
                    let Some(LocalInit { expr, .. }) = &local_stmt.init else {
                        continue;
                    };
                    // check some expression types, which can yield type of variable
                    match expr.as_ref() {
                        Expr::Struct(expr_struct) => {
                            if let Ok(PathElement::Item(node)) =
                                self.graph.get_path_leaf(self.node, expr_struct.into())
                            {
                                self.variables.push_variable(ident.to_owned(), node);
                                num_variables += 1;
                            }
                        }
                        Expr::Path(expr_path) => {
                            // at current state we expect a path to
                            // 1. an enum variant
                            // 2. another variable in scope, which could be an enum, a struct or an union
                            // 3. a const or a const inside an impl block
                            for path_element in SourcePathWalker::with_variables(
                                expr_path.into(),
                                self.node,
                                self.variables.clone(),
                            )
                            .into_iter(self.graph)
                            {
                                match path_element {
                                    PathElement::Group
                                    | PathElement::Glob(_)
                                    | PathElement::ItemRenamed(_, _) => {
                                        unreachable!("Not possible in syn path")
                                    }
                                    PathElement::ExternalPackage
                                    | PathElement::PathCouldNotBeParsed => break,
                                    PathElement::Item(node) => {
                                        let const_type = if let Some(item) =
                                            self.graph.get_syn_item(node)
                                        {
                                            match item {
                                                Item::Enum(_)
                                                | Item::Struct(_)
                                                | Item::Union(_) => {
                                                    self.variables
                                                        .push_variable(ident.to_owned(), node);
                                                    num_variables += 1;
                                                    break;
                                                }
                                                Item::Const(item_const) => &item_const.ty,
                                                _ => continue,
                                            }
                                        } else if let Some(impl_item) =
                                            self.graph.get_syn_impl_item(node)
                                        {
                                            let ImplItem::Const(impl_item_const) = impl_item else {
                                                continue;
                                            };
                                            &impl_item_const.ty
                                        } else {
                                            continue;
                                        };
                                        if let Type::Path(type_path) = const_type {
                                            if let Ok(PathElement::Item(node)) = self
                                                .graph
                                                .get_path_leaf(self.node, type_path.into())
                                            {
                                                // set variable type to type of const
                                                self.variables
                                                    .push_variable(ident.to_owned(), node);
                                                num_variables += 1;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Expr::Call(expr_call) => {
                            // At current state only parsing a simple path to constructor like new()
                            // or a alone standing fn(), both represented as Expr::Path(ExprPath) inside ExprCall.
                            // ToDo: support builder pattern
                            if let Expr::Path(expr_path_of_method) = expr_call.func.as_ref() {
                                if let Ok(PathElement::Item(fn_or_method_node)) = self
                                    .graph
                                    .get_path_leaf(self.node, expr_path_of_method.into())
                                {
                                    let output = if let Some(Item::Fn(item_fn)) =
                                        self.graph.get_syn_item(fn_or_method_node)
                                    {
                                        Some(&item_fn.sig.output)
                                    } else if let Some(ImplItem::Fn(impl_item_fn)) =
                                        self.graph.get_syn_impl_item(fn_or_method_node)
                                    {
                                        Some(&impl_item_fn.sig.output)
                                    } else {
                                        None
                                    };

                                    if let Some(ReturnType::Type(_, box_type)) = output {
                                        if let Type::Path(type_path) = box_type.as_ref() {
                                            if let Ok(PathElement::Item(node)) = self
                                                .graph
                                                .get_path_leaf(fn_or_method_node, type_path.into())
                                            {
                                                // set variable type to return type of method call
                                                self.variables
                                                    .push_variable(ident.to_owned(), node);
                                                num_variables += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ => (),
                    }
                }
                _ => (),
            }
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
            SourcePathWalker::with_variables(path.into(), self.node, self.variables.clone())
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

    fn visit_expr_method_call(&mut self, expr_method_call: &'a ExprMethodCall) {
        if let Expr::Path(expr_path) = expr_method_call.receiver.as_ref() {
            if let SourcePath::Name(segments) = expr_path.into() {
                if segments.len() == 1 {
                    let item_node = if segments[0] == "self" {
                        // get item node which is referenced by self
                        self.graph
                            .get_parent_index_by_edge_type(self.node, EdgeType::Syn)
                            .and_then(|n| {
                                self.graph
                                    .get_parent_index_by_edge_type(n, EdgeType::Implementation)
                            })
                    } else {
                        // check if receiver is listed in variables
                        self.variables.get_node_index(&segments[0])
                    };

                    // add reference to method call, if receiver is self or listed in variables
                    if let Some(node) = item_node {
                        for (impl_method, _) in self
                            .graph
                            .iter_impl_blocks_of_item(node)
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
        }
        // recursive visit
        syn::visit::visit_expr_method_call(self, expr_method_call);
    }
}

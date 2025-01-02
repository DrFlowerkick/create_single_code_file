// functions to visit the challenge tree

use petgraph::{
    stable_graph::NodeIndex,
    visit::{Bfs, EdgeRef},
};
// petgraph uses FixedBitSet as VisitMap for Bfs
use fixedbitset::FixedBitSet;
use syn::Item;

use super::{ChallengeTree, EdgeType, NodeType};

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

pub struct BfsModuleNameSpace {
    walker: Bfs<NodeIndex, FixedBitSet>,
    start: NodeIndex,
}

impl BfsModuleNameSpace {
    pub fn new(graph: &ChallengeTree, start: NodeIndex) -> Self {
        Self {
            walker: Bfs::new(graph, start),
            start,
        }
    }

    pub fn into_iter(self, graph: &ChallengeTree) -> BfsIterator<'_, BfsModuleNameSpace> {
        BfsIterator {
            walker: self,
            graph,
        }
    }
}

impl BfsWalker for BfsModuleNameSpace {
    // code adapted from petgraph, see Bfs implementation of next()
    fn next(&mut self, graph: &ChallengeTree) -> Option<NodeIndex> {
        if let Some(node) = self.walker.stack.pop_front() {
            let add_successors =
                if let Some(NodeType::SynItem(Item::Impl(item_impl))) = graph.node_weight(node) {
                    item_impl.trait_.is_none()
                } else {
                    self.start == node
                };
            // add only successors, which are connected by syn edge type and when add_successors is true
            for successor in graph
                .edges(node)
                .filter(|e| add_successors && *e.weight() == EdgeType::Syn)
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

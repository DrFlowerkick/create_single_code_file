// functions to visit the challenge tree

use petgraph::{
    graph::NodeIndex,
    visit::{Bfs, EdgeRef},
};
// petgraph uses FixedBitSet as VisitMap for Bfs
use fixedbitset::FixedBitSet;

use super::{ChallengeTree, EdgeType};

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

    // code adapted from petgraph, see Bfs implementation of next()
    pub fn next(&mut self, graph: &ChallengeTree) -> Option<NodeIndex> {
        if let Some(node) = self.walker.stack.pop_front() {
            // add only successors, which are connected by specified edge type
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

    pub fn into_iter(self, graph: &ChallengeTree) -> BfsByEdgeTypeIterator<'_> {
        BfsByEdgeTypeIterator {
            walker: self,
            graph,
        }
    }
}

pub struct BfsByEdgeTypeIterator<'a> {
    walker: BfsByEdgeType,
    graph: &'a ChallengeTree,
}

impl<'a> Iterator for BfsByEdgeTypeIterator<'a> {
    type Item = NodeIndex;

    fn next(&mut self) -> Option<Self::Item> {
        self.walker.next(self.graph)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            self.walker.walker.stack.len(),
            Some(self.graph.node_count()),
        )
    }
}

// iterator fn for the challenge tree

use super::{BfsByEdgeType, EdgeType, LocalPackage, NodeType, SrcFile};
use crate::{CgData, configuration::CgCli};
use either::Either;
use petgraph::{Direction, stable_graph::NodeIndex, visit::EdgeRef};
use syn::{ImplItem, Item, TraitItem};

impl<O, S> CgData<O, S> {
    pub(crate) fn iter_packages(&self) -> impl Iterator<Item = (NodeIndex, &NodeType)> {
        BfsByEdgeType::new(&self.tree, 0.into(), EdgeType::Dependency)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .fuse()
    }

    pub(crate) fn iter_local_packages(&self) -> impl Iterator<Item = (NodeIndex, &LocalPackage)> {
        self.iter_packages().filter_map(|(n, w)| match w {
            NodeType::LocalPackage(package) => Some((n, package)),
            NodeType::ExternalSupportedPackage(_) | NodeType::ExternalUnsupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    pub(crate) fn iter_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &NodeType)> {
        // skip first element, which is root of tree and therefore not a dependency
        self.iter_packages().skip(1)
    }

    pub(crate) fn iter_accepted_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.iter_dependencies().filter_map(|(n, w)| match w {
            NodeType::LocalPackage(local_package) => Some((n, local_package.name.as_str())),
            NodeType::ExternalSupportedPackage(name) => Some((n, name.as_str())),
            NodeType::ExternalUnsupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    pub(crate) fn iter_unsupported_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.iter_dependencies().filter_map(|(n, w)| match w {
            NodeType::ExternalUnsupportedPackage(name) => Some((n, name.as_str())),
            NodeType::LocalPackage(_) | NodeType::ExternalSupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    pub(crate) fn iter_external_dependencies(&self) -> impl Iterator<Item = &str> {
        // include elements of rust libraries in iterator
        self.iter_dependencies()
            .filter_map(|(_, w)| match w {
                NodeType::ExternalSupportedPackage(name)
                | NodeType::ExternalUnsupportedPackage(name) => Some(name.as_str()),
                NodeType::LocalPackage(_) => None,
                _ => unreachable!("Dependency edges only target package nodes."),
            })
            .chain(["std", "core", "std"])
    }

    pub(crate) fn iter_package_crates(
        &self,
        package_index: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, bool, &SrcFile)> {
        BfsByEdgeType::new(&self.tree, package_index, EdgeType::Crate)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| match w {
                NodeType::BinCrate(bin_crate_file) => Some((n, false, bin_crate_file)),
                NodeType::LibCrate(lib_crate_file) => Some((n, true, lib_crate_file)),
                _ => None,
            })
            .fuse()
    }

    pub(crate) fn iter_crates(&self) -> impl Iterator<Item = (NodeIndex, bool, &SrcFile)> {
        self.iter_local_packages()
            .flat_map(|(pi, _)| self.iter_package_crates(pi))
    }

    pub(crate) fn iter_lib_crates(&self) -> impl Iterator<Item = (NodeIndex, &SrcFile)> {
        self.iter_local_packages().filter_map(|(n, _)| {
            self.iter_package_crates(n)
                .filter_map(|(n, crate_type, cf)| if crate_type { Some((n, cf)) } else { None })
                .next()
        })
    }

    pub(crate) fn iter_impl_blocks_of_item(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &Item)> {
        self.tree
            .edges_directed(node, Direction::Outgoing)
            .filter(|e| *e.weight() == EdgeType::Implementation)
            .map(|e| e.target())
            .filter_map(|n| self.get_syn_item(n).map(|i| (n, i)))
    }

    pub(crate) fn iter_syn_neighbors(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &NodeType)> {
        self.tree
            .edges_directed(node, Direction::Outgoing)
            .filter(|e| *e.weight() == EdgeType::Syn)
            .map(|e| e.target())
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
    }

    pub(crate) fn iter_syn(&self, node: NodeIndex) -> impl Iterator<Item = (NodeIndex, &NodeType)> {
        BfsByEdgeType::new(&self.tree, node, EdgeType::Syn)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .fuse()
    }

    pub(crate) fn iter_syn_of_crate_or_module(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &NodeType)> {
        if !self.is_crate_or_module(node) {
            return Either::Left(std::iter::empty());
        }
        Either::Right(
            self.iter_syn_item_neighbors(node)
                .flat_map(|(n, i)| match i {
                    Item::Mod(_) => Either::Left(std::iter::empty()),
                    Item::Impl(_) => Either::Right(Either::Left(Either::Left(
                        self.iter_syn_impl_item(n).map(|(n, _)| n),
                    ))),
                    Item::Trait(_) => Either::Right(Either::Left(Either::Right(
                        self.iter_syn_trait_item(n).map(|(n, _)| n),
                    ))),
                    _ => Either::Right(Either::Right(std::iter::once(n))),
                })
                .filter_map(|n| self.tree.node_weight(n).map(|nt| (n, nt))),
        )
    }

    pub(crate) fn iter_syn_item_neighbors(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &Item)> {
        self.iter_syn_neighbors(node).filter_map(|(n, w)| match w {
            NodeType::SynItem(item) => Some((n, item)),
            _ => None,
        })
    }

    pub(crate) fn iter_syn_items(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &Item)> {
        self.iter_syn(node).filter_map(|(n, w)| match w {
            NodeType::SynItem(syn_item) => Some((n, syn_item)),
            _ => None,
        })
    }

    pub(crate) fn iter_syn_impl_item(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &ImplItem)> {
        self.iter_syn_neighbors(node).filter_map(|(n, w)| match w {
            NodeType::SynImplItem(impl_item) => Some((n, impl_item)),
            _ => None,
        })
    }

    pub(crate) fn iter_syn_trait_item(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &TraitItem)> {
        self.iter_syn_neighbors(node).filter_map(|(n, w)| match w {
            NodeType::SynTraitItem(trait_item) => Some((n, trait_item)),
            _ => None,
        })
    }

    pub(crate) fn iter_items_required_by_challenge(
        &self,
    ) -> impl Iterator<Item = (NodeIndex, &NodeType)> {
        self.iter_crates()
            .flat_map(|(n, _, _)| self.iter_syn(n))
            .filter(|(n, _)| self.is_required_by_challenge(*n))
    }
}

impl<O: CgCli, S> CgData<O, S> {
    pub(crate) fn iter_impl_blocks_without_required_link_of_required_items(
        &self,
    ) -> impl Iterator<Item = (NodeIndex, NodeIndex)> + use<'_, O, S> {
        self.get_required_crates_and_modules_sorted_by_relevance()
            .into_iter()
            .flat_map(|v| v.into_iter())
            .flat_map(|n| {
                self.iter_syn_item_neighbors(n)
                    .filter(|(n, _)| self.is_required_by_challenge(*n))
            })
            .flat_map(move |(rn, _)| {
                self.iter_impl_blocks_of_item(rn).filter_map(move |(n, _)| {
                    (!self.is_required_by_challenge(n)).then_some((n, rn))
                })
            })
    }

    pub(crate) fn iter_impl_items_without_required_link_in_required_impl_blocks(
        &self,
    ) -> impl Iterator<Item = (NodeIndex, NodeIndex)> + use<'_, O, S> {
        self.get_required_crates_and_modules_sorted_by_relevance()
            .into_iter()
            .flat_map(|v| v.into_iter())
            .flat_map(|n| self.iter_syn_item_neighbors(n))
            .flat_map(|(n, _)| {
                self.iter_impl_blocks_of_item(n)
                    .filter(|(n, _)| self.is_required_by_challenge(*n))
            })
            .flat_map(move |(rn, _)| {
                self.iter_syn_impl_item(rn).filter_map(move |(n, _)| {
                    (!self.is_required_by_challenge(n)
                        && !self
                            .get_possible_usage_of_impl_item_in_required_items(n)
                            .is_empty())
                    .then_some((n, rn))
                })
            })
    }
}

// functions to navigate the challenge tree

use super::{
    visit::BfsByEdgeType, ChallengeTreeError, CrateFile, EdgeType, LocalPackage, NodeTyp,
    TreeResult,
};
use crate::{add_context, configuration::CliInput, CgData};

use anyhow::Context;
use petgraph::{graph::NodeIndex, visit::EdgeRef, Direction};
use syn::Item;

impl<O, S> CgData<O, S> {
    pub fn challenge_package(&self) -> &LocalPackage {
        if let NodeTyp::LocalPackage(ref package) = self.tree.node_weight(0.into()).unwrap() {
            return package;
        }
        unreachable!("Challenge package is created at instantiation of CgDate and should always be at index 0.");
    }

    pub fn link_to_package(&mut self, source: NodeIndex, target: NodeIndex) {
        self.tree.add_edge(source, target, EdgeType::Dependency);
    }

    pub fn get_local_package(&self, node: NodeIndex) -> TreeResult<&LocalPackage> {
        if let NodeTyp::LocalPackage(dependency) = self
            .tree
            .node_weight(node)
            .ok_or_else(|| ChallengeTreeError::IndexError(node))?
        {
            Ok(dependency)
        } else {
            Err(ChallengeTreeError::NotLocalPackage(node))
        }
    }

    pub fn get_binary_crate(&self, node: NodeIndex) -> TreeResult<&CrateFile> {
        if let NodeTyp::BinCrate(crate_file) = self
            .tree
            .node_weight(node)
            .ok_or_else(|| ChallengeTreeError::IndexError(node))?
        {
            Ok(crate_file)
        } else {
            Err(ChallengeTreeError::NotBinaryCrate(node))
        }
    }

    pub fn get_library_crate(&self, node: NodeIndex) -> TreeResult<&CrateFile> {
        if let NodeTyp::LibCrate(crate_file) = self
            .tree
            .node_weight(node)
            .ok_or_else(|| ChallengeTreeError::IndexError(node))?
        {
            Ok(crate_file)
        } else {
            Err(ChallengeTreeError::NotLibraryCrate(node))
        }
    }

    fn iter_packages(&self) -> impl Iterator<Item = (NodeIndex, &NodeTyp)> {
        BfsByEdgeType::new(&self.tree, 0.into(), EdgeType::Dependency)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .fuse()
    }

    pub fn iter_local_packages(&self) -> impl Iterator<Item = (NodeIndex, &LocalPackage)> {
        self.iter_packages().filter_map(|(n, w)| match w {
            NodeTyp::LocalPackage(package) => Some((n, package)),
            NodeTyp::ExternalSupportedPackage(_) | NodeTyp::ExternalUnsupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    fn iter_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &NodeTyp)> {
        // skip first element, which is root of tree and therefore not a dependency
        self.iter_packages().skip(1)
    }

    pub fn iter_accepted_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.iter_dependencies().filter_map(|(n, w)| match w {
            NodeTyp::LocalPackage(local_package) => Some((n, local_package.name.as_str())),
            NodeTyp::ExternalSupportedPackage(name) => Some((n, name.as_str())),
            NodeTyp::ExternalUnsupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    pub fn iter_unsupported_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.iter_dependencies().filter_map(|(n, w)| match w {
            NodeTyp::ExternalUnsupportedPackage(name) => Some((n, name.as_str())),
            NodeTyp::LocalPackage(_) | NodeTyp::ExternalSupportedPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
    }

    pub fn iter_external_dependencies(&self) -> impl Iterator<Item = &str> {
        // include elements of rust libraries in iterator
        self.iter_dependencies()
            .filter_map(|(_, w)| match w {
                NodeTyp::ExternalSupportedPackage(name)
                | NodeTyp::ExternalUnsupportedPackage(name) => Some(name.as_str()),
                NodeTyp::LocalPackage(_) => None,
                _ => unreachable!("Dependency edges only target package nodes."),
            })
            .chain(["std", "core", "std"])
    }

    fn iter_package_crates(
        &self,
        package_index: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, bool, &CrateFile)> {
        BfsByEdgeType::new(&self.tree, package_index, EdgeType::Crate)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| match w {
                NodeTyp::BinCrate(bin_crate_file) => Some((n, false, bin_crate_file)),
                NodeTyp::LibCrate(lib_crate_file) => Some((n, true, lib_crate_file)),
                _ => None,
            })
            .fuse()
    }

    pub fn get_challenge_lib_crate(&self) -> Option<(NodeIndex, &CrateFile)> {
        self.iter_package_crates(0.into())
            .filter_map(|(n, crate_type, cf)| if crate_type { Some((n, cf)) } else { None })
            .next()
    }

    pub fn iter_lib_crates(&self) -> impl Iterator<Item = (NodeIndex, &CrateFile)> {
        self.iter_local_packages().filter_map(|(n, _)| {
            self.iter_package_crates(n)
                .filter_map(|(n, crate_type, cf)| if crate_type { Some((n, cf)) } else { None })
                .next()
        })
    }

    pub fn iter_syn_neighbors(&self, node: NodeIndex) -> impl Iterator<Item = (NodeIndex, &Item)> {
        self.tree
            .edges_directed(node, Direction::Outgoing)
            .filter(|e| *e.weight() == EdgeType::Syn)
            .map(|e| e.target())
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| match w {
                NodeTyp::SynItem(item) => Some((n, item)),
                _ => unreachable!("All syn edges must end in SynItem nodes."),
            })
    }

    pub fn iter_syn_items(&self, node: NodeIndex) -> impl Iterator<Item = (NodeIndex, &Item)> {
        BfsByEdgeType::new(&self.tree, node, EdgeType::Syn)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| match w {
                NodeTyp::SynItem(syn_item) => Some((n, syn_item)),
                _ => None,
            })
            .fuse()
    }

    pub fn get_syn_item_source_index(&self, node: NodeIndex) -> Option<NodeIndex> {
        self.tree
            .edges_directed(node, Direction::Incoming)
            .filter(|e| *e.weight() == EdgeType::Syn)
            .map(|e| e.source())
            .next()
    }

    pub fn get_syn_item(&self, node: NodeIndex) -> Option<&Item> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeTyp::SynItem(item) => Some(item),
            _ => None,
        })
    }

    pub fn get_name_of_crate_or_module(&self, node: NodeIndex) -> Option<String> {
        self.tree.node_weight(node).and_then(|w| match w {
            NodeTyp::SynItem(item) => {
                if let Item::Mod(item_mod) = item {
                    Some(item_mod.ident.to_string())
                } else {
                    None
                }
            }
            NodeTyp::BinCrate(crate_file) | NodeTyp::LibCrate(crate_file) => {
                Some(crate_file.name.to_owned())
            }
            _ => None,
        })
    }
    pub fn iter_syn_neighbors_without_semantic_link(
        &self,
        node: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &Item)> {
        self.iter_syn_neighbors(node).filter(move |(target, _)| {
            !self
                .tree
                .edges_connecting(node, *target)
                .any(|e| *e.weight() == EdgeType::Semantic)
        })
    }
}

impl<O: CliInput, S> CgData<O, S> {
    pub fn get_challenge_bin_name(&self) -> &str {
        if self.options.input().input == "main" {
            // if main, use crate name for bin name
            self.challenge_package().name.as_str()
        } else {
            self.options.input().input.as_str()
        }
    }

    pub fn get_challenge_bin_crate(&self) -> Option<(NodeIndex, &CrateFile)> {
        let bin_name = self.get_challenge_bin_name();
        self.iter_package_crates(0.into())
            .filter_map(|(n, crate_type, cf)| if !crate_type { Some((n, cf)) } else { None })
            .find(|(_, cf)| cf.name == bin_name)
    }

    pub fn get_crate_indices(&self, reverse: bool) -> TreeResult<Vec<NodeIndex>> {
        // get challenge bin and all lib crate indices
        let (bin_crate_index, _) = self
            .get_challenge_bin_crate()
            .context(add_context!("Expected challenge bin."))?;
        let mut crate_indices: Vec<NodeIndex> = self.iter_lib_crates().map(|(n, _)| n).collect();
        if reverse {
            // indices from end of dependency tree to challenge bin crate
            crate_indices.reverse();
            crate_indices.push(bin_crate_index);
        } else {
            // indices from challenge bin crate to end of dependency tree
            crate_indices.insert(0, bin_crate_index);
        }
        Ok(crate_indices)
    }
}

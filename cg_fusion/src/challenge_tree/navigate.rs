// functions to navigate the challenge tree

use super::{
    visit::BfsByEdgeType, ChallengeTreeError, CrateFile, EdgeType, LocalPackage, NodeTyp,
    TreeResult,
};
use crate::{configuration::CliInput, CgData};

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

    pub fn iter_external_dependencies(&self) -> impl Iterator<Item = (NodeIndex, &str)> {
        self.iter_dependencies().filter_map(|(n, w)| match w {
            NodeTyp::ExternalSupportedPackage(name) | NodeTyp::ExternalUnsupportedPackage(name) => {
                Some((n, name.as_str()))
            }
            NodeTyp::LocalPackage(_) => None,
            _ => unreachable!("Dependency edges only target package nodes."),
        })
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

    pub fn iter_syn_items(&self, node: NodeIndex) -> impl Iterator<Item = (NodeIndex, &Item)> {
        self.tree
            .edges_directed(node, Direction::Outgoing)
            .filter(|e| *e.weight() == EdgeType::Syn)
            .map(|e| e.target())
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| match w {
                // ignore empty Verbatim, which is created when 'mod tests' is removed, see load_syntax()
                NodeTyp::SynItem(item) => match item {
                    Item::Verbatim(verb) => {
                        if verb.is_empty() {
                            None
                        } else {
                            Some((n, item))
                        }
                    }
                    _ => Some((n, item)),
                },
                _ => unreachable!("All syn edges must end in SynItem nodes."),
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
}

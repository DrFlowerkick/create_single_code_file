// Building a tree of challenge and dependencies src files

mod error;
pub use error::{ChallengeTreeError, TreeResult};

mod visit;
pub use visit::BfsByEdgeType;

use cargo_metadata::camino::Utf8PathBuf;
use petgraph::graph::NodeIndex;
use std::cell::RefCell;
use syn::File;

use crate::{configuration::CliInput, metadata::MetaWrapper, parsing::load_syntax, CgData};

#[derive(Debug)]
pub enum NodeTyp {
    LocalPackage(LocalPackage),
    ExternalSupportedPackage(String),
    ExternalUnsupportedPackage(String),
    BinCrate(CrateFile),
    LibCrate(CrateFile),
    Module(ModuleFile),
}

#[derive(Debug)]
pub struct LocalPackage {
    pub name: String,
    pub path: Utf8PathBuf,
    pub metadata: Box<MetaWrapper>,
}

#[derive(Debug)]
pub struct CrateFile {
    pub name: String,
    pub path: Utf8PathBuf,
    pub syntax: RefCell<File>,
}

#[derive(Debug)]
pub struct ModuleFile {
    pub name: String,
    pub path: Utf8PathBuf,
    pub crate_index: NodeIndex,
    pub syntax: RefCell<File>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EdgeType {
    Dependency,
    Crate,
    Module,
    Uses,
}

impl TryFrom<MetaWrapper> for LocalPackage {
    type Error = ChallengeTreeError;

    fn try_from(value: MetaWrapper) -> Result<Self, Self::Error> {
        let metadata = Box::new(value);
        Ok(Self {
            name: metadata.package_name()?.to_owned(),
            path: metadata.package_root_dir()?,
            metadata,
        })
    }
}

impl TryFrom<cargo_metadata::Metadata> for LocalPackage {
    type Error = ChallengeTreeError;

    fn try_from(value: cargo_metadata::Metadata) -> Result<Self, Self::Error> {
        Self::try_from(MetaWrapper::new(value))
    }
}

// generic implementations for CgData concerning the challenge_tree
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

    pub fn get_module(&self, node: NodeIndex) -> TreeResult<&ModuleFile> {
        if let NodeTyp::Module(module_file) = self
            .tree
            .node_weight(node)
            .ok_or_else(|| ChallengeTreeError::IndexError(node))?
        {
            Ok(module_file)
        } else {
            Err(ChallengeTreeError::NotModule(node))
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

    pub fn iter_dependencies_lib_crates(&self) -> impl Iterator<Item = (NodeIndex, &CrateFile)> {
        // skip first local package, which is root of tree (challenge package)
        self.iter_local_packages().skip(1).filter_map(|(n, _)| {
            self.iter_package_crates(n)
                .filter_map(|(n, crate_type, cf)| if crate_type { Some((n, cf)) } else { None })
                .next()
        })
    }

    pub fn iter_modules(
        &self,
        crate_index: NodeIndex,
    ) -> impl Iterator<Item = (NodeIndex, &ModuleFile)> {
        BfsByEdgeType::new(&self.tree, crate_index, EdgeType::Module)
            .into_iter(&self.tree)
            .filter_map(|n| self.tree.node_weight(n).map(|w| (n, w)))
            .filter_map(|(n, w)| match w {
                NodeTyp::Module(module_file) => Some((n, module_file)),
                _ => None,
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

    pub fn add_local_package(&mut self, source: NodeIndex, package: LocalPackage) -> NodeIndex {
        if self.options.verbose() {
            println!(
                "Found local dependency '{}' at '{}'",
                package.name, package.path
            );
        }
        let package_index = self.tree.add_node(NodeTyp::LocalPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
        package_index
    }

    pub fn add_external_supported_package(
        &mut self,
        source: NodeIndex,
        package: String,
    ) -> NodeIndex {
        if self.options.verbose() {
            println!("Found external supported dependency '{}'", package);
        }
        let package_index = self
            .tree
            .add_node(NodeTyp::ExternalSupportedPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
        package_index
    }

    pub fn add_external_unsupported_package(
        &mut self,
        source: NodeIndex,
        package: String,
    ) -> NodeIndex {
        if self.options.verbose() {
            println!("Found external unsupported dependency '{}'", package);
        }
        let package_index = self
            .tree
            .add_node(NodeTyp::ExternalUnsupportedPackage(package));
        self.tree
            .add_edge(source, package_index, EdgeType::Dependency);
        package_index
    }

    pub fn add_binary_crate_to_package(
        &mut self,
        package_node_index: NodeIndex,
        name: String,
    ) -> TreeResult<NodeIndex> {
        // get bin path from metadata
        let path = self
            .get_local_package(package_node_index)?
            .metadata
            .get_binary_target_of_root_package(name.as_str())?
            .src_path
            .to_owned();

        // get syntax of src file
        let syntax = load_syntax(&path)?;
        // generate node value
        let crate_file = CrateFile {
            name,
            path,
            syntax: RefCell::new(syntax),
        };

        if self.options.verbose() {
            println!(
                "Adding binary crate '{}' with path '{}' to tree...",
                crate_file.name, crate_file.path
            );
        }

        let crate_node_index = self.tree.add_node(NodeTyp::BinCrate(crate_file));
        self.tree
            .add_edge(package_node_index, crate_node_index, EdgeType::Crate);

        Ok(crate_node_index)
    }

    pub fn add_library_crate_to_package(
        &mut self,
        package_node_index: NodeIndex,
    ) -> TreeResult<Option<NodeIndex>> {
        // get bin path from metadata
        if let Some(target) = self
            .get_local_package(package_node_index)?
            .metadata
            .get_library_target_of_root_package()?
        {
            // get syntax of src file
            let syntax = load_syntax(&target.src_path)?;
            // generate node value
            let crate_file = CrateFile {
                name: target.name.to_owned(),
                path: target.src_path.to_owned(),
                syntax: RefCell::new(syntax),
            };

            if self.options.verbose() {
                println!(
                    "Adding library crate '{}' with path '{}' to tree...",
                    crate_file.name, crate_file.path
                );
            }

            let crate_node_index = self.tree.add_node(NodeTyp::LibCrate(crate_file));
            self.tree
                .add_edge(package_node_index, crate_node_index, EdgeType::Crate);

            Ok(Some(crate_node_index))
        } else {
            Ok(None)
        }
    }

    pub fn add_module(
        &mut self,
        name: String,
        path: Utf8PathBuf,
        source_index: NodeIndex,
        crate_index: NodeIndex,
    ) -> TreeResult<NodeIndex> {
        // get syntax of src file
        let mod_syntax = load_syntax(&path)?;

        if self.options.verbose() {
            println!("found module '{}', adding {} to tree...", name, path);
        }

        // create node value
        let module_src_file = ModuleFile {
            name,
            path,
            crate_index,
            syntax: RefCell::new(mod_syntax.clone()),
        };

        let module_node_index = self.tree.add_node(NodeTyp::Module(module_src_file));
        self.tree
            .add_edge(source_index, module_node_index, EdgeType::Module);
        Ok(module_node_index)
    }
}

// analysis of dependencies

use super::{AnalyzeError, AnalyzeState};
use crate::{
    challenge_tree::{BfsByEdgeType, BfsWalker, EdgeType, LocalPackage},
    configuration::{ChallengePlatform, CliInput},
    metadata::MetaWrapper,
    utilities::CODINGAME_SUPPORTED_CRATES,
    CgData,
};

use petgraph::graph::NodeIndex;

impl<O: CliInput> CgData<O, AnalyzeState> {
    pub fn add_challenge_dependencies(&mut self) -> Result<(), AnalyzeError> {
        // borrow checker requires taking ownership of dependencies for adding new nodes and edges to self.tree
        let dependencies = self
            .challenge_package()
            .metadata
            .root_package()?
            .dependencies
            .to_owned();
        // add all direct dependencies of challenge package to tree
        for dep in dependencies.iter() {
            if let Some(ref local_path) = dep.path {
                let dep_toml = local_path.join("Cargo.toml");
                let metadata = MetaWrapper::try_from(&dep_toml)?;
                let dependency = LocalPackage::try_from(metadata)?;
                // add dependency to tree
                self.add_local_package(0.into(), dependency);
            } else {
                let dep_name = dep.name.to_owned();
                if self.iter_supported_crates().any(|c| c == dep_name.as_str()) {
                    // found supported package, add to tree
                    self.add_external_supported_package(0.into(), dep_name);
                } else {
                    return Err(AnalyzeError::CodingameUnsupportedDependencyOfChallenge(
                        dep_name,
                    ));
                }
            }
        }
        // check direct dependencies of challenge for further dependencies
        let mut dependency_walker = BfsByEdgeType::new(&self.tree, 0.into(), EdgeType::Dependency);
        // skip first element, which is challenge
        dependency_walker.next(&self.tree);
        while let Some(dependency_node) = dependency_walker.next(&self.tree) {
            self.analyze_challenge_sub_dependencies(dependency_node)?;
        }

        if self.options.verbose() {
            println!("Running 'cargo check' and 'cargo clippy' for local packages...");
        }
        for (_, package) in self.iter_local_packages() {
            package
                .metadata
                .run_cargo_check()?
                .collect_cargo_check_messages()?;
            package
                .metadata
                .run_cargo_clippy()?
                .collect_cargo_clippy_messages()?;
        }
        Ok(())
    }

    fn analyze_challenge_sub_dependencies(&mut self, node: NodeIndex) -> Result<(), AnalyzeError> {
        // check for root packages and get dependencies
        // borrow checker requires taking ownership of dependencies for adding new nodes and edges to self.tree
        let dependencies = match self.get_local_package(node)?.metadata.root_package() {
            Ok(root_packages) => root_packages.dependencies.to_owned(),
            // if there is no root packages, there should be a workspace
            Err(_) => Vec::new(),
        };
        // check dependencies of local dependency, if there are any
        for dep in dependencies.iter() {
            if let Some(ref local_path) = dep.path {
                // if dependency is already in tree, get index of node otherwise None.
                let dependency_node = self
                    .iter_accepted_dependencies()
                    .find(|(_, name)| *name == dep.name)
                    .map(|(n, _)| n);
                // if Some(n), dependency is already in tree, therefore return node index, otherwise create new node
                // has to be done in two steps because of borrow checker
                match dependency_node {
                    Some(n) => self.link_to_package(node, n),
                    None => {
                        let dep_toml = local_path.join("Cargo.toml");
                        let metadata = MetaWrapper::try_from(&dep_toml)?;
                        let dependency = LocalPackage::try_from(metadata)?;
                        let dependency_node = self.add_local_package(node, dependency);
                        // recursive call for checking dependencies of dependency
                        self.analyze_challenge_sub_dependencies(dependency_node)?;
                    }
                }
            } else {
                let dep_name = dep.name.to_owned();
                if self.iter_supported_crates().any(|c| c == dep_name) {
                    // found supported package, which is not dependency of challenge
                    if self.options.force() {
                        self.add_external_supported_package(node, dep_name);
                    } else {
                        return Err(AnalyzeError::DependencyOfLocalLibraryIsNotIncludedInDependenciesOfChallenge(
                            dep_name,
                        ));
                    }
                } else {
                    // found unsupported package
                    if self.options.force() {
                        // if dependency is already in tree, get index of node otherwise None.
                        let dependency_node = self
                            .iter_unsupported_dependencies()
                            .find(|(_, name)| *name == dep_name)
                            .map(|(n, _)| n);
                        match dependency_node {
                            Some(n) => self.link_to_package(node, n),
                            None => {
                                self.add_external_unsupported_package(node, dep_name);
                            }
                        }
                    } else {
                        return Err(AnalyzeError::CodingameUnsupportedDependencyOfLocalLibrary(
                            dep_name,
                        ));
                    }
                }
            }
        }

        // check for workspace packages
        let members = self
            .get_local_package(node)?
            .metadata
            .get_member_manifests_of_workspace();
        for (member_name, member_path) in members.iter() {
            // if dependency is already in tree, get index of node or None.
            let dependency_node = self
                .iter_accepted_dependencies()
                .find(|(_, name)| *name == member_name)
                .map(|(n, _)| n);
            // if Some(n), dependency is already in tree, therefore return node index, otherwise create new node
            // has to be done in two steps because of borrow checker
            match dependency_node {
                Some(n) => self.link_to_package(node, n),
                None => {
                    let metadata = MetaWrapper::try_from(member_path)?;
                    let dependency = LocalPackage::try_from(metadata)?;
                    let dependency_node = self.add_local_package(node, dependency);
                    // recursive call for checking dependencies of dependency
                    self.analyze_challenge_sub_dependencies(dependency_node)?;
                }
            }
        }

        Ok(())
    }

    fn iter_supported_crates(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        match self.options.input().platform {
            ChallengePlatform::Codingame => Box::new(CODINGAME_SUPPORTED_CRATES.into_iter()),
            ChallengePlatform::Other => Box::new(
                self.options
                    .input()
                    .other_supported_crates
                    .iter()
                    .map(|c| c.as_str()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::super::tests::setup_analyze_test;

    #[test]
    fn test_collecting_dependencies() {
        let mut cg_data = setup_analyze_test();
        cg_data.add_challenge_dependencies().unwrap();
        let dependencies: Vec<&str> = cg_data
            .iter_accepted_dependencies()
            .map(|(_, n)| n)
            .collect();
        assert_eq!(
            dependencies,
            vec!["cg_fusion_lib_test", "my_map_two_dim", "my_array", "rand"]
        );
    }
}

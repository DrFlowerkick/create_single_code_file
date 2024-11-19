// error definitions for metadata

use crate::error::error_chain_fmt;
use cargo_metadata::camino::Utf8PathBuf;

#[derive(thiserror::Error)]
pub enum MetadataError {
    #[error("Failed to execute MetadataCommand.")]
    ErrorMetadataCommand(#[from] cargo_metadata::Error),
    #[error("No root package in metadata of challenge crate.")]
    NoRootPackage,
    #[error("Error handling manifest path '{0}' in metadata.")]
    ErrorManifestPathOfMetadata(Utf8PathBuf),
    #[error("Could not find binary '{0}' in root package of metadata.")]
    BinaryNotFound(String),
    #[error("Error executing 'cargo check' command.")]
    CargoCheckError(#[from] std::io::Error),
    #[error("Codingame does not support '{0}'.")]
    CodingameUnsupportedDependencyOfChallenge(String),
    #[error("Codingame does not support '{0}', use '--force' to ignore.")]
    CodingameUnsupportedDependencyOfLocalLibrary(String),
    #[error(
        "Dependency of local library '{0}' is not in dependencies of challenge, \
         use '--force' to ignore or add '{0}' as dependency to challenge."
    )]
    DependencyOfLocalLibraryIsNotIncludedInDependenciesOfChallenge(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for MetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

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
}

impl std::fmt::Debug for MetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

// error definitions for challenge tree

use crate::{error::error_chain_fmt, metadata::MetadataError, parsing::ParsingError};
use cargo_metadata::camino::Utf8PathBuf;
use petgraph::graph::NodeIndex;

pub type TreeResult<T> = Result<T, ChallengeTreeError>;

#[derive(thiserror::Error)]
pub enum ChallengeTreeError {
    #[error("Something went wrong with using Metadata of challenge crate.")]
    MetadataError(#[from] MetadataError),
    #[error("Something went wrong with reading file content.")]
    ReadFromFileError(#[from] std::io::Error),
    #[error("Something went wrong with parsing a source file.")]
    ParsingError(#[from] ParsingError),
    #[error("Something went wrong with reading file content.")]
    SerdeConvertError(#[from] toml::de::Error),
    #[error("Something went wrong converting PathBuf to Utf8PathBuf")]
    FromPathBufError(#[from] cargo_metadata::camino::FromPathBufError),
    #[error("Tree node does not contain index '{:?}'.", .0)]
    IndexError(NodeIndex),
    #[error("Tree node does not contain local package at index '{:?}'.", .0)]
    NotLocalPackage(NodeIndex),
    #[error("Tree node does not contain binary crate at index '{:?}'.", .0)]
    NotBinaryCrate(NodeIndex),
    #[error("Tree node does not contain library crate at index '{:?}'.", .0)]
    NotLibraryCrate(NodeIndex),
    #[error("Tree node does not contain crate or syn items at index '{:?}'.", .0)]
    NotCrateOrSyn(NodeIndex),
    #[error("Configured impl item '{0}' of input option does not exist.")]
    NotExistingImplItemOfConfig(String),
    #[error("Configured impl item '{0}' is not unique. Add fully qualified impl block name (see --help).")]
    NotUniqueImplItem(String),
    #[error("Invalid impl config option '{0}' (see --help).")]
    InvalidImplConfigOption(String),
    #[error("Path '{:?}' is not inside challenge dir.", .0)]
    NotInsideChallengeDir(Utf8PathBuf),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ChallengeTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

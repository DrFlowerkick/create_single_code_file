// error definitions for challenge tree

use crate::{error::error_chain_fmt, metadata::MetadataError, parsing::ParsingError};
use petgraph::graph::NodeIndex;

pub type TreeResult<T> = Result<T, ChallengeTreeError>;

#[derive(thiserror::Error)]
pub enum ChallengeTreeError {
    #[error("Something went wrong with using Metadata of challenge crate.")]
    MetadataError(#[from] MetadataError),
    #[error("Something went wrong with parsing a source file.")]
    ParsingError(#[from] ParsingError),
    #[error("Tree node does not contain index '{:?}'.", 0)]
    IndexError(NodeIndex),
    #[error("Tree node does not contain local package at index '{:?}'.", 0)]
    NotLocalPackage(NodeIndex),
    #[error("Tree node does not contain binary crate at index '{:?}'.", 0)]
    NotBinaryCrate(NodeIndex),
    #[error("Tree node does not contain library crate at index '{:?}'.", 0)]
    NotLibraryCrate(NodeIndex),
    #[error("Tree node does not contain module at index '{:?}'.", 0)]
    NotModule(NodeIndex),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ChallengeTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

// error definitions for challenge tree

use crate::{error::error_chain_fmt, metadata::MetadataError};

pub type TreeResult<T> = Result<T, ChallengeTreeError>;

#[derive(thiserror::Error)]
pub enum ChallengeTreeError {
    #[error("Something went wrong with using Metadata of challenge crate.")]
    MetadataError(#[from] MetadataError),
    #[error("Tree node does not contain a local package.")]
    NotLocalPackage,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ChallengeTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

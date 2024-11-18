// error definitions for challenge tree

use crate::{error::error_chain_fmt, metadata::MetadataError};

pub type TreeResult<T> = Result<T, ChallengeTreeError>;

#[derive(thiserror::Error)]
pub enum ChallengeTreeError {
    #[error("Something went wrong with using Metadata of challenge crate.")]
    MetadataError(#[from] MetadataError),
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

impl std::fmt::Debug for ChallengeTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

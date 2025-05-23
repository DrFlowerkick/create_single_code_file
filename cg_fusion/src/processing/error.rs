// error definitions for processing

use crate::{
    challenge_tree::ChallengeTreeError, error::error_chain_fmt, metadata::MetadataError,
    parsing::ParsingError,
};

pub type ProcessingResult<T> = Result<T, ProcessingError>;

#[derive(thiserror::Error)]
pub enum ProcessingError {
    #[error("Something went wrong with using Metadata of challenge crate.")]
    MetadataError(#[from] MetadataError),
    #[error("Something went wrong with parsing a source file.")]
    ParsingError(#[from] ParsingError),
    #[error("Something went wrong with fmt.")]
    FmtError(#[from] std::fmt::Error),
    #[error("Something went wrong with io.")]
    IoError(#[from] std::io::Error),
    #[error("Something went wrong with toml editing.")]
    TomlError(#[from] toml_edit::TomlError),
    #[error("Something went wrong with using the challenge tree.")]
    ChallengeTreeError(#[from] ChallengeTreeError),
    #[error("Codingame does not support '{0}', use '--force' to ignore.")]
    CodingameUnsupportedDependency(String),
    #[error(
        "Dependency of local library '{0}' is not in dependencies of challenge, \
         use '--force' to ignore or add '{0}' as dependency to challenge."
    )]
    DependencyOfLocalLibraryIsNotIncludedInDependenciesOfChallenge(String),
    #[error("Maximum number of attempts to expand use statement '{0}' in module '{1}'.")]
    MaxAttemptsExpandingUseStatement(String, String),
    #[error("Some use statements could not be parsed.")]
    UseStatementsCouldNotBeParsed,
    #[error("Dialog canceled by user.")]
    UserCanceledDialog,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ProcessingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

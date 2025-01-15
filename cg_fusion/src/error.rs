// error and result definitions

use crate::{
    processing::ProcessingError, challenge_tree::ChallengeTreeError, metadata::MetadataError,
    parsing::ParsingError,
};

use std::path::PathBuf;

pub type CgResult<T> = Result<T, CgError>;

pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

#[derive(thiserror::Error)]
pub enum CgError {
    #[error("Something went wrong during processing challenge and library src files.")]
    ProcessingError(#[from] ProcessingError),
    #[error("Something went wrong with using Metadata of challenge crate.")]
    MetadataError(#[from] MetadataError),
    #[error("Something went wrong with the Challenge Tree.")]
    ChallengeTreeError(#[from] ChallengeTreeError),
    #[error("Something went wrong with parsing.")]
    ParsingError(#[from] ParsingError),
    #[error("Not existing input file '{}' or filename is not ./src/main.rs .", .0.display())]
    MustProvideValidInputFilePath(PathBuf),
    #[error("Invalid output file name '{0}': file does not exist or is identical to input or does not end on '.rs'.")]
    MustProvideValidOutputFileName(String),
    #[error("Input file path '{}' points to invalid package structure.", .0.display())]
    PackageStructureError(PathBuf),
    #[error("Could not find start line of name space for message line {0}.")]
    NoStartLine(usize),
    #[error("Could not find end line of name space.")]
    NoEndLine,
    #[error("More closing brackets than starting brackets for name space.")]
    TooManyClosingBrackets,
    #[error("Could not find enum name of never constructed variant.")]
    CouldNotFindEnumName,
    #[error("Output mode accepts only 'merge' or 'update'.")]
    NotAcceptedOutputMode,
    #[error("Platform mode accepts only 'codingame' or 'other'.")]
    NotAcceptedPlatform,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

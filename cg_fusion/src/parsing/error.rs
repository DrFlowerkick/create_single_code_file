// error definitions for challenge tree

use crate::error::error_chain_fmt;

pub type ParsingResult<T> = Result<T, ParsingError>;

#[derive(thiserror::Error)]
pub enum ParsingError {
    #[error("Something went wrong with reading file content.")]
    ReadFromFileError(#[from] std::io::Error),
    #[error("Something went wrong with parsing file content.")]
    ParsingFileContentError(#[from] syn::parse::Error),
    #[error("Parsed file contains verbatim elements:\n{0}")]
    ContainsVerbatim(String),
    #[error("Only SourcePath::Name can be converted to Path.")]
    ConvertSourcePathToPathError,
    #[error("SourcePath::Group cannot be converted to UseTree.")]
    ConvertSourcePathGroupToUseTreeError,
    #[error("Not enough segments in SourcePath to convert to UseTree.")]
    ConvertSourcePathToUseTreeNotEnoughSegmentsError,
    #[error("Error converting SourcePath to SourceLeaf")]
    ConvertSourcePathToSourceLeafError,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

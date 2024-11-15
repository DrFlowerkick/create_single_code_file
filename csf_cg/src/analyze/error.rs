// error definitions for analyze

use crate::error::error_chain_fmt;

#[derive(thiserror::Error)]
pub enum AnalyzeError {
    #[error("Some analyze error")]
    SomeAnalyzeError,
    #[error("Solve remaining 'cargo check' messages before continuing:\n{0}")]
    RemainingCargoCheckMessagesOfInput(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for AnalyzeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

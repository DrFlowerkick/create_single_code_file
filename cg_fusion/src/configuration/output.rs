// output options of cli

use clap::Args;
use std::fmt::{self, Display};

#[derive(Debug, Args)]
pub struct OutputOptions {
    /// Filename of merged challenge src file without rs extension. Default filename
    /// is 'fusion_of_name_of_challenge_crate'. Challenge src file will be saved
    /// in './src/bin/' of the challenge crate. If this output file already exists
    /// you must use '-f' or '--force' to overwrite it.
    #[arg(
        short = 'n',
        long,
        help = "Filename of merged challenge src file without rs extension."
    )]
    pub filename: Option<String>,
}

impl Display for OutputOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "filename: {:?}", self.filename)
    }
}

#[cfg(test)]
impl Default for OutputOptions {
    fn default() -> Self {
        Self { filename: None }
    }
}

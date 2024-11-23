// common options of cli

use clap::Args;
use std::fmt::{self, Display};

#[derive(Debug, Args)]
pub struct CommonOptions {
    /// Print verbose information during execution.
    #[arg(short, long, help = "Print verbose information during execution.")]
    pub verbose: bool,

    #[command(flatten)]
    pub manifest: clap_cargo::Manifest,

    /// Force overwriting existing output file. Force although fusion of challenge src files
    /// in case of missing dependencies from crate.io of a local library crate in challenge
    /// crate manifest.
    #[arg(
        short,
        long,
        help = "Force fusion and overwriting existing output file."
    )]
    pub force: bool,
}

impl Display for CommonOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "verbose: {}", self.verbose)
    }
}

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
}

impl Display for CommonOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "verbose: {}", self.verbose)
    }
}

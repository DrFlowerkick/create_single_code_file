// configuration options

mod common;
mod fusion;
mod input;
mod output;
mod processing;
mod traits;

use cargo_metadata::MetadataCommand;
pub use common::CommonOptions;
pub use fusion::FusionCli;
pub use input::{ChallengePlatform, InputOptions};
pub use output::OutputOptions;
pub use processing::ProcessingOptions;
pub use traits::{CgCli, CgCliImplDialog};

use clap::Parser;

// CargoCli enum allows to extend cg-fusion for other user modes (e.G. a TUI) in the future
#[derive(Parser)]
#[command(
    name = "cargo",
    bin_name = "cargo",
    styles = clap_cargo::style::CLAP_STYLING,
    term_width = 100,
)]
pub enum CargoCli {
    CgFusion(FusionCli),
}

impl CargoCli {
    pub fn metadata_command(&self) -> MetadataCommand {
        match self {
            CargoCli::CgFusion(fusion_cli) => fusion_cli.manifest_metadata_command(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        CargoCli::command().debug_assert();
    }
}

// configuration options

mod analyze;
mod common;
mod fusion;
mod input;
mod merge;
mod output;
mod purge;
mod traits;

pub use analyze::AnalyzeCli;
use cargo_metadata::MetadataCommand;
pub use common::CommonOptions;
pub use fusion::FusionCli;
pub use input::{InputOptions, Mode};
pub use merge::{MergeCli, MergeOptions};
pub use output::OutputOptions;
pub use purge::{PurgeCli, PurgeOptions};
pub use traits::{CliCommon, CliInput, CliMerge, CliOutput, CliPurge};

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "cargo",
    bin_name = "cargo",
    styles = clap_cargo::style::CLAP_STYLING,
    term_width = 100,
)]
pub enum CargoCli {
    CgFusion(FusionCli),
    CgAnalyze(AnalyzeCli),
    CgMerge(MergeCli),
    CgPurge(PurgeCli),
}

impl CargoCli {
    pub fn delete_tmp_files(&self) -> Option<bool> {
        match self {
            CargoCli::CgFusion(fusion_cli) => Some(!fusion_cli.output().keep_tmp_file),
            CargoCli::CgAnalyze(_) => None,
            CargoCli::CgMerge(merge_cli) => Some(!merge_cli.output().keep_tmp_file),
            CargoCli::CgPurge(purge_cli) => Some(!purge_cli.output().keep_tmp_file),
        }
    }

    pub fn metadata_command(&self) -> MetadataCommand {
        match self {
            CargoCli::CgFusion(fusion_cli) => fusion_cli.manifest_metadata_command(),
            CargoCli::CgAnalyze(analyze_cli) => analyze_cli.manifest_metadata_command(),
            CargoCli::CgMerge(merge_cli) => merge_cli.manifest_metadata_command(),
            CargoCli::CgPurge(purge_cli) => purge_cli.manifest_metadata_command(),
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

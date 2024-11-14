// purge options of cli

use super::{CliCommon, CliOutput, CliPurge, CommonOptions, OutputOptions};

use clap::Args;
use std::fmt::{self, Display};

#[derive(Debug, Args)]
pub struct PurgeOptions {
    /// Max number of purge cycles. Purging of of code spans reported by compiler messages works in cycles.
    /// Each cycle runs 'cargo check' on crate directory, therefore checking the newly created binary of
    /// merged challenge code. First it checks for Errors. If no Errors exist, it checks for Warnings. If
    /// no Warnings exist, purging is finished. All entries are saved in a BTreeMap and than resolved step
    /// by step from bottom to top of the temporary output file. Afterward a new cycle starts. This option
    /// sets the max number of cycles. Minimum is one cycle.
    #[arg(
        short = 'y',
        long,
        default_value_t = 1000,
        value_parser = clap::value_parser!(u16).range(1..),
        help = "Max number of purge cycles.",
    )]
    pub max_purge_cycles: u16,

    /// Max number of purge steps per purgecycle. Each purge cycle consists of a number of compiler messages
    /// sorted by line of occurrence inside a BTreeMap. Each compiler message counts as one step. The steps
    /// are resolved from bottom to top of of temporary output file. If the maximum number of steps is
    /// reached, purging is stopped and current output is saved. This option sets the max number of steps.
    /// Minimum is one step.
    #[arg(
        short = 'x',
        long,
        default_value_t = 1000,
        value_parser = clap::value_parser!(u16).range(1..),
        help = "Max number of purge steps per purge cycle.",
    )]
    pub max_steps_per_cycle: u16,
}

impl Display for PurgeOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "max-purge-cycles: {}", self.max_purge_cycles)?;
        writeln!(f, "max-steps-per-cycle: {}", self.max_steps_per_cycle)
    }
}

#[derive(Debug, Args)]
#[command(
    version,
    about,
    long_about = "cg-purge executes the purge part of cg-fusion. For fine control of \
                  execution and in-depth analysis of purge results use debug mode, \
                  set 'max-purge-cycles' to 1, keep the temporary files, and run cg-purge as often \
                  as is required to finalize the purge process indicated by a 'purge finished' message. \
                  This will result in one temporary file per cycle starting with '001' as extension and \
                  counting up ('000' is reserved for original merged output file). In case of an error, \
                  the last file processed by cg-purge will have an 'rs' extension. Otherwise the \
                  last temporary file will be copied to configured output file."
)]
pub struct PurgeCli {
    #[command(flatten)]
    common_cli: CommonOptions,

    #[command(flatten)]
    output_cli: OutputOptions,

    #[command(flatten)]
    purge_cli: PurgeOptions,
}

impl Display for PurgeCli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.common_cli)?;
        writeln!(f, "{}", self.output_cli)?;
        writeln!(f, "{}", self.purge_cli)
    }
}

impl CliCommon for PurgeCli {
    fn verbose(&self) -> bool {
        self.common_cli.verbose
    }
    fn manifest_metadata_command(&self) -> cargo_metadata::MetadataCommand {
        self.common_cli.manifest.metadata()
    }
}

impl CliOutput for PurgeCli {
    fn output(&self) -> &OutputOptions {
        &self.output_cli
    }
}

impl CliPurge for PurgeCli {
    fn purge(&self) -> &PurgeOptions {
        &self.purge_cli
    }
}

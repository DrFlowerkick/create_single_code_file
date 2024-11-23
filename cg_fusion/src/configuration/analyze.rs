// analyze cli options

use super::{CliCommon, CliInput, CommonOptions, InputOptions};

use clap::Args;
use std::fmt::{self, Display};

#[derive(Debug, Args)]
#[command(
    version,
    about,
    long_about = "cg-analyze executes only the analyze part of cg-fusion. You can check in a \
                  dry-run which dependencies will be pulled in and decide, which of these \
                  should be blocked. cg-analyze will automatically set '-v' or '--verbose'."
)]
pub struct AnalyzeCli {
    #[command(flatten)]
    common_cli: CommonOptions,

    #[command(flatten)]
    input_cli: InputOptions,
}

impl Display for AnalyzeCli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.common_cli)?;
        writeln!(f, "{}", self.input_cli)
    }
}

impl CliCommon for AnalyzeCli {
    fn verbose(&self) -> bool {
        self.common_cli.verbose
    }
    fn manifest_metadata_command(&self) -> cargo_metadata::MetadataCommand {
        self.common_cli.manifest.metadata()
    }
    fn force(&self) -> bool {
        self.common_cli.force
    }
}

impl CliInput for AnalyzeCli {
    fn input(&self) -> &InputOptions {
        &self.input_cli
    }
}

impl AnalyzeCli {
    pub fn force_verbose(&mut self) {
        self.common_cli.verbose = true;
    }
}

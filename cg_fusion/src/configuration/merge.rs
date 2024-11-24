// merge options of cli

use super::{CliCommon, CliInput, CliMerge, CliOutput, CommonOptions, InputOptions, OutputOptions};

use clap::Args;
use std::fmt::{self, Display};

#[derive(Debug, Args)]
pub struct MergeOptions {
    /// Keep comments in merged src file.
    #[arg(short = 'c', long, help = "Keep comments in merged src file.")]
    pub keep_comments: bool,

    /// Do not delete empty lines in merged src file.
    #[arg(
        short = 'e',
        long,
        help = "Do not delete empty lines in merged src file."
    )]
    pub keep_empty_lines: bool,
}

impl Display for MergeOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "keep-comments: {}", self.keep_comments)?;
        writeln!(f, "keep-empty-lines: {}", self.keep_empty_lines)
    }
}

#[cfg(test)]
impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            keep_comments: false,
            keep_empty_lines: false,
        }
    }
}

#[derive(Debug, Args)]
#[command(
    version,
    about,
    long_about = "cg-merge executes the analyze and merge parts of cg-fusion. It creates \
                  an output file, which contains input src file and all of it's dependencies. \
                  Blocked indirect dependencies are of course not merged in. By default, all \
                  comments and empty lines are removed from merged output file.\n\n\
                  In debug mode cg-merge creates a temporary file as merge target. The filename \
                  is a uuid, with extension of '000.rs'. Normally the temporary file will be \
                  deleted. But if you want to execute the fusion process step-by-step by running \
                  first cg-merge followed by cg-purge, you should keep the temporary files for \
                  analysis in case of unexpected results."
)]
pub struct MergeCli {
    #[command(flatten)]
    common_cli: CommonOptions,

    #[command(flatten)]
    input_cli: InputOptions,

    #[command(flatten)]
    output_cli: OutputOptions,

    #[command(flatten)]
    merge_cli: MergeOptions,
}

impl Display for MergeCli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.common_cli)?;
        writeln!(f, "{}", self.input_cli)?;
        writeln!(f, "{}", self.output_cli)?;
        writeln!(f, "{}", self.merge_cli)
    }
}

impl CliCommon for MergeCli {
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

impl CliInput for MergeCli {
    fn input(&self) -> &InputOptions {
        &self.input_cli
    }
}

impl CliOutput for MergeCli {
    fn output(&self) -> &OutputOptions {
        &self.output_cli
    }
}

impl CliMerge for MergeCli {
    fn merge(&self) -> &MergeOptions {
        &self.merge_cli
    }
}

#[cfg(test)]
impl Default for MergeCli {
    fn default() -> Self {
        Self {
            common_cli: CommonOptions::default(),
            input_cli: InputOptions::default(),
            output_cli: OutputOptions::default(),
            merge_cli: MergeOptions::default(),
        }
    }
}

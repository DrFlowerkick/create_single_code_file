// fusion cli options

use super::{
    CliCommon, CliInput, CliMerge, CliOutput, CliPurge, CommonOptions, InputOptions, MergeOptions,
    OutputOptions, PurgeOptions,
};

use clap::Args;
use std::fmt::{self, Display};

#[derive(Debug, Args)]
#[command(
    version,
    about,
    long_about = "cg-fusion is a handy extension of cargo for codingame and similar online \
                  challenges, which require a single source file containing all required code. \
                  codingame supports the rust std library and some crates from crates.io (As of \
                  2024/11/13, the following packages are supported: chrono 0.4.26, itertools 0.11.0, \
                  libc 0.2.147, rand 0.8.5, regex 1.8.4, and time 0.3.22). Since sane programmers use \
                  code libraries for reusable and modular code, the requirements of codingame are \
                  counter intuitive for good practices. To participate at codingame and use a local \
                  code library all required src files have to be merged in one clumpy file and than \
                  purged from unused dead library code. Here comes cg-fusion into play by doing \
                  exactly that.\n\n\
                  Run cg-fusion with 'cargo cg-fusion' in the root directory of your challenge crate (as \
                  is normal for all cargo commands). By default cg-fusion takes 'main.rs' and analyzes \
                  it for all dependencies inside the crate and a local library, if applicable. If there \
                  are no dependencies cg-fusion does nothing. Otherwise cg-fusion will merge all required \
                  files into one src file and saves it by default as 'fusion_of_name_of_challenge_crate.rs' \
                  inside of 'challenge_crate_dir/src/bin/'. Normally this merged file contains \
                  unwanted code fragments, which either prevent it from compilation or is dead code, \
                  which just takes up space. Therefore the next step of cg-fusion is to call repeatedly \
                  'cargo check' and purging step by step unwanted code fragments. This process is \
                  unstable and may result in in broken code. If you want to have more information about \
                  merge and purge results, use 'd' or '--debug'. In debug mode cg-fusion creates temporary \
                  files inside of 'crate_dir/src/bin/' for the initial merged file and for each purge_cycle. \
                  Analyze these files if you get unexpected results from cg_fusion. If no error occurs, the \
                  final temporary file is copied to the target location and than all temporary files deleted. \
                  There are some options to control this behavior. cargo-cg-fusion also provides additional \
                  cargo extensions to execute analyze, merge, and purge in separate steps for more fine \
                  control of the process.\n\n\
                  One warning about crates.io dependencies: cg-fusion does not pull these dependencies \
                  and merge them into the challenge file. If the challenge code does depend upon other \
                  crates than 'rand', the challenge code my not work on codingame. If the local library \
                  has dependencies to crate.io, which are not fulfilled by the challenge crate, cg-fusion \
                  shows a warning and does not process the challenge code. In this case either add these \
                  dependencies to the challenge crate (if the are required by the challenge code) or use \
                  'cargo cg-fusion --force' to proceed."
)]
pub struct FusionCli {
    #[command(flatten)]
    common_cli: CommonOptions,

    #[command(flatten)]
    input_cli: InputOptions,

    #[command(flatten)]
    output_cli: OutputOptions,

    #[command(flatten)]
    merge_cli: MergeOptions,

    #[command(flatten)]
    purge_cli: PurgeOptions,
}

impl Display for FusionCli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.common_cli)?;
        writeln!(f, "{}", self.input_cli)?;
        writeln!(f, "{}", self.output_cli)?;
        writeln!(f, "{}", self.merge_cli)?;
        writeln!(f, "{}", self.purge_cli)
    }
}

impl CliCommon for FusionCli {
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

impl CliInput for FusionCli {
    fn input(&self) -> &InputOptions {
        &self.input_cli
    }
}

impl CliOutput for FusionCli {
    fn output(&self) -> &OutputOptions {
        &self.output_cli
    }
}

impl CliMerge for FusionCli {
    fn merge(&self) -> &MergeOptions {
        &self.merge_cli
    }
}

impl CliPurge for FusionCli {
    fn purge(&self) -> &PurgeOptions {
        &self.purge_cli
    }
}

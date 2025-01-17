// fusion cli options

use super::{CgCli, CgCliImplDialog, CommonOptions, InputOptions, OutputOptions};

use clap::Args;
use std::fmt::{self, Display};

#[derive(Debug, Args)]
#[command(
    version,
    about,
    long_about = "cg-fusion is a handy extension of cargo for codingame and similar online \
                  challenges, which require a single source file containing all necessary code. \
                  codingame supports the rust std library and some crates from crates.io (As of \
                  2025/01/15, the following packages are supported: rust 1.70 chrono 0.4.26, \
                  itertools 0.11.0, libc 0.2.147, rand 0.8.5, regex 1.8.4, and time 0.3.22). Since \
                  sane programmers use code libraries for reusable and modular code, the requirements \
                  of codingame are counter intuitive for good practices. To participate at codingame \
                  and use a local code library all required src files respectively all required src \
                  items have to be merged in one clumpy file. Here comes cg-fusion into play by doing \
                  exactly that.\n\n\
                  Run cg-fusion with 'cargo cg-fusion' in the root directory of your challenge crate (as \
                  is normal for all cargo commands). By default cg-fusion takes 'main.rs' and analyzes \
                  it for all dependencies inside the crate and a local library, if applicable. If there \
                  are no dependencies cg-fusion does nothing. Otherwise cg-fusion will parse all src files \
                  for required src items of challenge fn main(). cg-fusion does not fully implement syntax \
                  and semantic of rust. Therefore it cannot fully automatically identify all required src \
                  items. Especially items of impl blocks need at current state user interaction to decide, \
                  which impl item to include and which to exclude from merged challenge bin. cg.fusion provides \
                  some options to provide information of processing impl items. With these information fully \
                  automatic processing and merging of challenge src files is possible. The merged src file is \
                  by default as 'fusion_of_name_of_challenge_crate.rs' inside of
                  'challenge_crate_dir/src/bin/'.\n\n\
                  One warning about crates.io dependencies: cg-fusion does not pull these dependencies \
                  and merge them into the challenge file. If the challenge code does depend upon other \
                  crates than supported by the challenge platform, the challenge code will probably not run on \
                  it. For codingame, the supported dependencies are hardcoded in cg-fusion. For other platforms \
                  supported crate names has to be declared via the option 'other-supported-crates'. If your  local \
                  library has dependencies to crate.io, which are not fulfilled by the challenge crate, cg-fusion \
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
}

impl Display for FusionCli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.common_cli)?;
        writeln!(f, "{}", self.input_cli)?;
        writeln!(f, "{}", self.output_cli)
    }
}

impl CgCli for FusionCli {
    fn verbose(&self) -> bool {
        self.common_cli.verbose
    }
    fn manifest_metadata_command(&self) -> cargo_metadata::MetadataCommand {
        self.common_cli.manifest.metadata()
    }
    fn force(&self) -> bool {
        self.common_cli.force
    }
    fn input(&self) -> &InputOptions {
        &self.input_cli
    }
    fn output(&self) -> &OutputOptions {
        &self.output_cli
    }
}

impl CgCliImplDialog for FusionCli {}

#[cfg(test)]
impl Default for FusionCli {
    fn default() -> Self {
        Self {
            common_cli: CommonOptions::default(),
            input_cli: InputOptions::default(),
            output_cli: OutputOptions::default(),
        }
    }
}

#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
impl FusionCli {
    pub fn set_manifest_path(&mut self, path: PathBuf) {
        self.common_cli.manifest.manifest_path = Some(path);
    }
    pub fn set_impl_include(&mut self, impl_items: Vec<String>) {
        self.input_cli.include_impl_item = impl_items;
    }
    pub fn set_impl_exclude(&mut self, impl_items: Vec<String>) {
        self.input_cli.exclude_impl_item = impl_items;
    }
}

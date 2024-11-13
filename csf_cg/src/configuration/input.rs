// input options of cli

use clap::{Args, ValueEnum};
use std::fmt::{self, Display};
use std::str::FromStr;

use crate::CGError;

#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Mode {
    /// Merge challenge src files with all of it's dependencies and create a new file.
    /// To overwrite an existing output file use '--force'.
    #[value(
        help = "Merge challenge src files with all of it's dependencies and create a new file. \
                To overwrite an existing output file use '--force'."
    )]
    Merge,

    /// Updates existing output file with configured components. Falls back to 'merge' if no file exists.
    #[value(help = "Updates existing output file with configured components. \
                Falls back to 'merge' if no file exists.")]
    Update,
}

impl FromStr for Mode {
    type Err = CGError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "merge" => Ok(Self::Merge),
            "update" => Ok(Self::Update),
            _ => Err(CGError::NotAcceptedOutputMode),
        }
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Merge => write!(f, "merge"),
            Mode::Update => write!(f, "update"),
        }
    }
}

#[derive(Debug, Args)]
pub struct InputOptions {
    /// Filename of input binary without rs extension.
    #[arg(
        short,
        long,
        default_value = "main",
        help = "Filename of input binary without rs extension."
    )]
    pub input: String,

    /// Mode of file fusion.
    #[arg(
        short = 'o',
        long,
        default_value_t = Mode::Merge,
        help = "Mode of file fusion.",
    )]
    pub mode: Mode,

    /// Select specific src files to update in already merged output file. Does only apply if
    /// mode set to 'update'. Use this option multiple times to add multiple src files to update.
    /// Use 'main' for main input file, 'challenge' for all src files of challenge crate, and
    /// specific module names for specific module src files. You can also use the crate name of
    /// a local library crate to add all dependencies of it to the update list.
    /// If new dependencies are detected, which have yet not been merged into output file, they
    /// will be merged as in 'merge' mode.
    #[arg(
        short,
        long,
        default_values = &["challenge"],
        help = "Select specific src files to update in already merged output file.",
    )]
    pub update_components: Vec<String>,

    /// If the challenge crate depends upon a local crate library, you can use this option to block
    /// unwanted indirect dependencies from the crate library. Library crates contain a lot of functions
    /// in separate modules and these functions may depend upon further modules of the library. If these
    /// modules are not referenced with a 'use' statement inside a challenge src file, they are called
    /// indirect modules. Some of these indirect modules may not be required by the challenge code.
    /// Block these unwanted indirect dependencies by using '-b name_of_module_to_block' as often as
    /// needed. Namespace path of module is only required bijective names must be ensured.
    /// This option increases speed of execution, since less unwanted code has to be purged by
    /// blocked it in advance.
    #[arg(
        short,
        long,
        help = "Block unwanted indirect dependencies from library crate."
    )]
    pub block_indirect: Vec<String>,
}

impl Display for InputOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "input: {}", self.input)?;
        writeln!(f, "mode: {}", self.mode)?;
        writeln!(f, "update-components: {:?}", self.update_components)?;
        writeln!(f, "block-indirect: {:?}", self.block_indirect)
    }
}

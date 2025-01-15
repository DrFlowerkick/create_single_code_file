// input options of cli

use clap::{Args, ValueEnum};
use std::fmt::{self, Display};
use std::path::PathBuf;
use std::str::FromStr;

use crate::CgError;

#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ChallengePlatform {
    /// Default platform is codingame. Supported crates of codingame or hardcoded in cg-fusion.
    #[value(
        help = "Default platform is codingame. Supported crates of codingame or hardcoded in cg-fusion."
    )]
    Codingame,

    /// Choose other for other platform. Add supported crates with '--other-supported-crates'.
    #[value(
        help = "Choose other for other platform. Add supported crates with '--other-supported-crates'."
    )]
    Other,
}

impl FromStr for ChallengePlatform {
    type Err = CgError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "codingame" => Ok(Self::Codingame),
            "other" => Ok(Self::Other),
            _ => Err(CgError::NotAcceptedPlatform),
        }
    }
}

impl Display for ChallengePlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChallengePlatform::Codingame => write!(f, "codingame"),
            ChallengePlatform::Other => write!(f, "other"),
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

    /// Either include or exclude all impl items:
    /// true:  include all impl items of all required impl blocks.
    /// false: exclude all impl items of all required impl blocks, which are not explicitly
    ///        required by challenge.
    /// If not set (shown as None), this option is ignored.
    ///
    /// If in conflict with other impl options, the 'include' option always wins.
    #[arg(short = 'r', long, help = "Either include or exclude all impl items.")]
    pub process_all_impl_items: Option<bool>,

    /// Select specific impl items of specific user defined types to include in challenge.
    /// naming convention:
    /// optional_crate_name::optional_module_name_i::user_defined_type_name::impl_item_name
    /// Crate and module names are only required, if the name of the user defined type is
    /// ambiguous.
    ///
    /// If in conflict with other impl options, the 'include' option always wins.
    #[arg(
        short = 'j',
        long,
        help = "Select specific impl items of specific user defined types to include in challenge."
    )]
    pub include_impl_item: Vec<String>,

    /// Select specific impl items of specific user defined types to exclude from challenge.
    /// naming convention:
    /// optional_crate_name::optional_module_name_i::user_defined_type_name::impl_item_name
    /// Crate and module names are only required, if the name of the user defined type is
    /// ambiguous.
    ///
    /// If in conflict with other impl options, the 'include' option always wins.
    #[arg(
        short = 'x',
        long,
        help = "Select specific impl items of specific user defined types to exclude from challenge."
    )]
    pub exclude_impl_item: Vec<String>,

    /// Path of config file in TOML format to configure included or excluded impl items of
    /// specific user defined types in respectively from challenge.
    /// file structure:
    /// [impl_item]
    /// include_impl_items = [include_item_1, include_item_2]
    /// exclude_impl_items = [exclude_item_1, exclude_item_2]
    ///
    /// naming convention of items:
    /// optional_crate_name::optional_module_name_i::user_defined_type_name::impl_item_name
    /// Crate and module names are only required, if the name of the user defined type is
    /// ambiguous.
    ///
    /// If in conflict with other impl options, the 'include' option always wins.
    #[arg(
        short = 't',
        long,
        help = "Path of config file in TOML format to configure included or excluded impl items of \
                specific user defined types in respectively from challenge."
    )]
    pub impl_item_toml: Option<PathBuf>,

    /// Challenge platform the fusion is made for.
    #[arg(
        short = 'p',
        long,
        default_value_t = ChallengePlatform::Codingame,
        help = "Challenge platform the fusion is made for.",
    )]
    pub platform: ChallengePlatform,

    /// Supported crates of other challenge platform. Use multiple times to append multiple values.
    #[arg(
        short = 's',
        long,
        requires = "platform",
        help = "Supported crates of other challenge platform. Use multiple times to append multiple values."
    )]
    pub other_supported_crates: Vec<String>,
}

impl Display for InputOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "input: {}", self.input)?;
        writeln!(
            f,
            "process-all-impl-items: {:?}",
            self.process_all_impl_items
        )?;
        writeln!(f, "include-impl-item: {:?}", self.include_impl_item)?;
        writeln!(f, "exclude-impl-item: {:?}", self.exclude_impl_item)?;
        writeln!(f, "impl-item-toml: {:?}", self.impl_item_toml)?;
        writeln!(f, "platform: {}", self.platform)?;
        writeln!(f, "block-indirect: {:?}", self.exclude_impl_item)
    }
}

#[cfg(test)]
impl Default for InputOptions {
    fn default() -> Self {
        Self {
            input: "main".into(),
            process_all_impl_items: None,
            include_impl_item: Vec::new(),
            exclude_impl_item: Vec::new(),
            impl_item_toml: None,
            platform: ChallengePlatform::Codingame,
            other_supported_crates: Vec::new(),
        }
    }
}

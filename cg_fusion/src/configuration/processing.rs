// options for processing the challenge files of cli

use clap::Args;
use std::fmt::{self, Display};
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct ProcessingOptions {
    /// If a use glob '*' points to a module, which itself has use globs, these use globs must
    /// be expanded first. A complex use glob structure, where multiple use globs depend on
    /// ech other may result in circular dependency in the way, cg-fusion tries to expand these
    /// use globs. To prevent hanging loops, cg-fusion tries to expand each use glob for a
    /// default maximum number of  five attempts. With 'glob-expansion-max-attempts' may be
    /// changed to a value between 0 and 255.
    #[arg(
        short,
        long,
        default_value = "5",
        help = "Max number of attempts to expand use globs."
    )]
    pub glob_expansion_max_attempts: u8,

    /// Either include or exclude all impl items:
    /// true:  include all impl items of all required impl blocks.
    /// false: exclude all impl items of all required impl blocks, which are not explicitly
    ///        required by challenge.
    /// If not set (shown as None), this option is ignored.
    ///
    /// If in conflict with other impl options, the option which 'include' the impl item always wins.
    #[arg(short = 'r', long, help = "Either include or exclude all impl items.")]
    pub process_all_impl_items: Option<bool>,

    /// Select specific impl items of specific user defined types to include in challenge.
    /// If the name of the impl item is ambiguous (e.g. push(), next(), etc.), add as much
    /// information to the name as is required to make the name unique including the name of
    /// the user defined type:
    /// path::to::module::of::impl_block_of_user_defined_type_name::user_defined_type_name::impl_item_name.
    ///
    /// Usage of wildcard '*' for impl item is possible, if at least the name of the user defined type is
    /// given. E.g. 'user_defined_type_name::*' will include all impl items of 'user_defined_type_name'.
    ///
    /// If in conflict with other impl options, the 'include' option always wins.
    #[arg(
        short = 'j',
        long,
        help = "Select specific impl items of specific user defined types to include in challenge."
    )]
    pub include_impl_item: Vec<String>,

    /// Select specific impl items of specific user defined types to exclude from challenge.
    /// If the name of the impl item is ambiguous (e.g. push(), next(), etc.), add as much
    /// information to the name as is required to make the name unique including the name of
    /// the user defined type:
    /// path::to::module::of::impl_block_of_user_defined_type_name::user_defined_type_name::impl_item_name.
    ///
    /// Usage of wildcard '*' for impl item is possible, if at least the name of the user defined type is
    /// given. E.g. 'user_defined_type_name::*' will exclude all impl items of 'user_defined_type_name'.
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
    /// include_impl_items = [include_item_1, include_item_2]
    /// exclude_impl_items = [exclude_item_1, exclude_item_2]
    ///
    /// If the name of the impl item is ambiguous (e.g. push(), next(), etc.), add as much
    /// information to the name as is required to make the name unique including the name of
    /// the user defined type:
    /// path::to::module::of::impl_block_of_user_defined_type_name::user_defined_type_name::impl_item_name.
    ///
    /// Usage of wildcard '*' for impl item is possible, if at least the name of the user defined type is
    /// given. E.g. 'user_defined_type_name::*' will include or exclude all impl items of
    /// 'user_defined_type_name'.
    ///
    /// If in conflict with other impl options, the 'include' option always wins.
    #[arg(
        short = 't',
        long,
        help = "Path of config file in TOML format to configure included or excluded impl items of \
                specific user defined types in respectively from challenge."
    )]
    pub impl_item_toml: Option<PathBuf>,
}

impl Display for ProcessingOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "glob-expansion-max-attempts: {}",
            self.glob_expansion_max_attempts
        )?;
        writeln!(
            f,
            "process-all-impl-items: {:?}",
            self.process_all_impl_items
        )?;
        writeln!(f, "include-impl-item: {:?}", self.include_impl_item)?;
        writeln!(f, "exclude-impl-item: {:?}", self.exclude_impl_item)?;
        writeln!(f, "impl-item-toml: {:?}", self.impl_item_toml)
    }
}

#[cfg(test)]
impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            glob_expansion_max_attempts: 5,
            process_all_impl_items: None,
            include_impl_item: Vec::new(),
            exclude_impl_item: Vec::new(),
            impl_item_toml: None,
        }
    }
}

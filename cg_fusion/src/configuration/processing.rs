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

    /// Either include or exclude impl blocks and items:
    /// true:  include all impl blocks and items of all required user defined types.
    /// false: exclude all impl blocks and items of all required user defined types,
    ///        which are not explicitly required by challenge.
    /// If not set (shown as None), this option is ignored.
    ///
    /// If in conflict with other impl options, the option which 'include' the impl item always wins.
    #[arg(short = 'r', long, help = "Either include or exclude all impl items.")]
    pub process_all_impl_items: Option<bool>,

    /// Path of config file in TOML format to configure impl items of specific impl blocks to
    /// include in or exclude from challenge.
    /// file structure:
    /// include_impl_items = [include_item_1, include_item_2]
    /// exclude_impl_items = [exclude_item_1, exclude_item_2]
    ///
    /// If the name of the impl item is ambiguous (e.g. push(), next(), etc.), add the fully
    /// qualified name of the impl block containing the impl item. Use the following naming
    /// schema:
    /// fully_qualified_name_of_impl_block::impl_item_name
    ///
    /// A fully qualified name of an impl block consists of two (no trait) or three (with trait)
    /// components:
    /// 1. impl with lifetime and type parameters if applicable, e.g. impl<'a,T:Display>
    /// 2. path to trait with lifetime and type parameters if applicable and 'for' keyword, e.g.
    ///    convert::From<&str> for
    /// 3. path to user defined type with lifetime and type parameters if applicable referenced by impl
    ///    block, e.g. map::TwoDim<X,Y>
    /// 
    /// Specify the components without any whitespace with the exception of one space between trait and
    /// 'for' keyword. The two or three parts are seperated by one space.
    /// Example 1: impl<X:usize,Y:usize> map::TwoDim<X,Y>
    /// Example 2: impl From<&str> for FooType
    ///
    /// Usage of wildcard '*' for impl item name is possible, but requires a fully qualified name of an
    /// impl block, e.g.: impl<X:usize,Y:usize> map::TwoDim<X,Y>::*
    /// This will include all impl item of the corresponding impl block(s)
    ///
    /// If in conflict with other impl options, the 'include' option always wins.
    #[arg(
        short = 't',
        long,
        help = "Path of config file in TOML format to configure impl items of specific impl blocks to \
                include in or exclude from challenge."
    )]
    pub impl_item_toml: Option<PathBuf>,

    /// Select specific impl items of specific impl blocks to include in challenge.
    ///
    /// For more information see -t --impl-item-toml
    #[arg(
        short = 'j',
        long,
        help = "Select specific impl items of specific impl blocks to include in challenge."
    )]
    pub include_impl_item: Vec<String>,

    /// Select specific impl items of specific impl blocks to exclude from challenge.
    ///
    /// For more information see -t --impl-item-toml
    #[arg(
        short = 'x',
        long,
        help = "Select specific impl items of specific impl blocks to exclude from challenge."
    )]
    pub exclude_impl_item: Vec<String>,
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
        writeln!(f, "impl-item-toml: {:?}", self.impl_item_toml)?;
        writeln!(f, "include-impl-item: {:?}", self.include_impl_item)?;
        writeln!(f, "exclude-impl-item: {:?}", self.exclude_impl_item)
    }
}

#[cfg(test)]
impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            glob_expansion_max_attempts: 5,
            process_all_impl_items: None,
            impl_item_toml: None,
            include_impl_item: Vec::new(),
            exclude_impl_item: Vec::new(),
        }
    }
}

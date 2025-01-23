// some utilities to use in project

use std::path::Path;

use anyhow::Result;
use cargo_metadata::camino::{absolute_utf8, Utf8PathBuf};
use path_clean::{clean, PathClean};
use relative_path::PathExt;

// macro for adding file and line info to messages
#[macro_export]
macro_rules! add_context {
    ($message:expr) => {
        format!("{} ({}:{})", $message, file!(), line!())
    };
}

// codingame supports the following crates from crates.io for rust 1.70
// see https://www.codingame.com/playgrounds/40701/help-center/languages-versions (2025-01-15)
// chrono 0.4.26, itertools 0.11.0, libc 0.2.147, rand 0.8.5, regex 1.8.4, time 0.3.22
// we ignore for now version numbers
pub const CODINGAME_SUPPORTED_CRATES: [&str; 6] =
    ["chrono", "itertools", "libc", "rand", "regex", "time"];

// get relative path from base to target
pub fn get_relative_path<P>(base_dir: P, target_path: P) -> Result<Utf8PathBuf>
where
    P: AsRef<Path>,
{
    let base_dir = clean_absolute_utf8(base_dir)?;
    let target_path = clean_absolute_utf8(target_path)?.as_std_path().to_owned();
    let relative_path = target_path.relative_to(base_dir)?.to_path(".").clean();
    Ok(Utf8PathBuf::try_from(relative_path)?)
}

// check if target_path is inside base_dir
pub fn is_inside_dir<P>(base_dir: P, target_path: P) -> Result<bool>
where
    P: AsRef<Path>,
{
    // convert to absolute path
    let base_dir = clean_absolute_utf8(base_dir)?;
    let target_path = clean_absolute_utf8(target_path)?;

    // check if target is part of base
    Ok(target_path.starts_with(&base_dir))
}

pub fn clean_absolute_utf8<P>(path: P) -> Result<Utf8PathBuf>
where
    P: AsRef<Path>,
{
    let clean_absolute_utf8 = Utf8PathBuf::try_from(clean(absolute_utf8(path)?))?;
    Ok(clean_absolute_utf8)
}

pub fn current_dir_utf8() -> Result<Utf8PathBuf> {
    let current_dir_utf8 = Utf8PathBuf::try_from(std::env::current_dir()?)?;
    Ok(current_dir_utf8)
}

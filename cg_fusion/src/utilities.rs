// some utilities to use in project

use anyhow::Result;
use cargo_metadata::camino::{absolute_utf8, Utf8PathBuf};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use inquire::{
    autocompletion::{Autocomplete, Replacement},
    ui::RenderConfig,
    validator::StringValidator,
    Confirm, CustomUserError, Select, Text,
};
use mockall::automock;
use path_clean::{clean, PathClean};
use relative_path::PathExt;
use std::{
    fmt::Display,
    fs,
    io::{ErrorKind, Write},
    path::Path,
};

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

// extension trait for Vec to drain elements based on a predicate
pub trait DrainFilterExt<T> {
    fn drain_filter<F>(&mut self, predicate: F) -> Vec<T>
    where
        F: Fn(&T) -> bool;
}

impl<T> DrainFilterExt<T> for Vec<T> {
    fn drain_filter<F>(&mut self, predicate: F) -> Vec<T>
    where
        F: Fn(&T) -> bool,
    {
        let mut extracted = Vec::new();
        let mut i = 0;

        while i < self.len() {
            if predicate(&self[i]) {
                extracted.push(self.swap_remove(i));
            } else {
                i += 1;
            }
        }

        extracted
    }
}

// trait to indicate that a type is sortable
pub trait Sortable {
    fn sort(&self, other: &Self) -> std::cmp::Ordering;
}

pub trait DrainFilterAndSortExt<T>: DrainFilterExt<T> {
    fn drain_filter_and_sort<F>(&mut self, predicate: F) -> Vec<T>
    where
        F: Fn(&T) -> bool,
        T: Sortable;
}

impl<T: Sortable> DrainFilterAndSortExt<T> for Vec<T> {
    fn drain_filter_and_sort<F>(&mut self, predicate: F) -> Vec<T>
    where
        F: Fn(&T) -> bool,
    {
        let mut extracted = self.drain_filter(predicate);
        extracted.sort_by(|a, b| a.sort(b));
        extracted
    }
}

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

// inquire dialog helper functions

#[automock]
pub trait CgDialog<S: Display + 'static, M: Display + 'static> {
    fn select_option(&self, prompt: &str, help: &str, options: Vec<S>) -> Result<Option<S>>;
    fn text_file_path<V: StringValidator + 'static>(
        &self,
        prompt: &str,
        help: &str,
        initial_value: &str,
        validator: V,
    ) -> Result<Option<Utf8PathBuf>>;
    fn confirm(&self, prompt: &str, help: &str, default_value: bool) -> Result<bool>;
    fn write_output(&mut self, message: M) -> Result<()>;
}

pub struct DialogCli<W: Write, S: Display + 'static, M: Display + 'static> {
    pub writer: W,
    _select_display_type: std::marker::PhantomData<S>,
    _message_display_type: std::marker::PhantomData<M>,
}

impl<W: Write> DialogCli<W, String, String> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            _select_display_type: std::marker::PhantomData,
            _message_display_type: std::marker::PhantomData,
        }
    }
}

impl<S: Display + 'static, M: Display + 'static, W: Write> CgDialog<S, M> for DialogCli<W, S, M> {
    fn select_option(&self, prompt: &str, help: &str, options: Vec<S>) -> Result<Option<S>> {
        let selected_item = Select::new(prompt, options)
            .with_render_config(RenderConfig::default_colored())
            .with_help_message(help)
            .prompt_skippable()?;
        Ok(selected_item)
    }

    fn text_file_path<V: StringValidator + 'static>(
        &self,
        prompt: &str,
        help: &str,
        initial_value: &str,
        validator: V,
    ) -> Result<Option<Utf8PathBuf>> {
        let file_path = Text::new(prompt)
            .with_render_config(RenderConfig::default_colored())
            .with_help_message(help)
            .with_initial_value(initial_value)
            .with_autocomplete(FilePathCompleter::default())
            .with_validator(validator)
            .prompt_skippable()?
            .map(Utf8PathBuf::from);
        Ok(file_path)
    }

    fn confirm(&self, prompt: &str, help: &str, default_value: bool) -> Result<bool> {
        let confirmation = Confirm::new(prompt)
            .with_render_config(RenderConfig::default_colored())
            .with_help_message(help)
            .with_default(default_value)
            .prompt()?;
        Ok(confirmation)
    }

    fn write_output(&mut self, message: M) -> Result<()> {
        write!(self.writer, "{}", message)?;
        Ok(())
    }
}

#[derive(Clone, Default)]
struct FilePathCompleter {
    input: String,
    paths: Vec<String>,
}

impl FilePathCompleter {
    fn update_input(&mut self, input: &str) -> Result<(), CustomUserError> {
        if input == self.input && !self.paths.is_empty() {
            return Ok(());
        }

        self.input = input.to_owned();
        self.paths.clear();

        let input_path = Utf8PathBuf::from(input);

        let fallback_parent = input_path
            .parent()
            .map(|p| {
                if *p == "" {
                    Utf8PathBuf::from(".")
                } else {
                    p.to_owned()
                }
            })
            .unwrap_or_else(|| Utf8PathBuf::from("."));

        let scan_dir = if input.ends_with('/') {
            input_path
        } else {
            fallback_parent.clone()
        };

        let entries = match fs::read_dir(&scan_dir) {
            Ok(read_dir) => Ok(read_dir),
            Err(err) if err.kind() == ErrorKind::NotFound => match fs::read_dir(&fallback_parent) {
                Ok(read_dir) => Ok(read_dir),
                Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()), // we accept non existing dirs
                Err(err) => Err(err),
            },
            Err(err) => Err(err),
        }?
        .collect::<Result<Vec<_>, _>>()?;

        for entry in entries {
            let path = entry.path();
            let path_str = if path.is_dir() {
                format!("{}/", path.to_string_lossy())
            } else {
                path.to_string_lossy().to_string()
            };

            self.paths.push(path_str);
        }

        Ok(())
    }

    fn fuzzy_sort(&self, input: &str) -> Vec<(String, i64)> {
        let mut matches: Vec<(String, i64)> = self
            .paths
            .iter()
            .filter_map(|path| {
                SkimMatcherV2::default()
                    .smart_case()
                    .fuzzy_match(path, input)
                    .map(|score| (path.clone(), score))
            })
            .collect();

        matches.sort_by(|a, b| b.1.cmp(&a.1));
        matches
    }
}

impl Autocomplete for FilePathCompleter {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError> {
        self.update_input(input)?;

        let matches = self.fuzzy_sort(input);
        Ok(matches.into_iter().take(15).map(|(path, _)| path).collect())
    }

    fn get_completion(
        &mut self,
        input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<Replacement, CustomUserError> {
        self.update_input(input)?;

        Ok(if let Some(suggestion) = highlighted_suggestion {
            Replacement::Some(suggestion)
        } else {
            let matches = self.fuzzy_sort(input);
            matches
                .first()
                .map(|(path, _)| Replacement::Some(path.clone()))
                .unwrap_or(Replacement::None)
        })
    }
}

// inquire dialog helper functions

use crate::add_context;
use anyhow::anyhow;
pub use anyhow::Result as AnyResult;
use cargo_metadata::camino::Utf8PathBuf;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use inquire::{
    autocompletion::{Autocomplete, Replacement},
    ui::RenderConfig,
    CustomUserError, Select, Text,
};
use mockall::automock;
use std::{
    fmt::Display,
    fs,
    io::{ErrorKind, Write},
};

#[derive(Debug, PartialEq, Eq)]
pub enum UserSelection {
    IncludeItem,
    ExcludeItem,
    IncludeAllItemsOfImplBlock,
    ExcludeAllItemsOfImplBlock,
    ShowItem,
    ShowUsageOfItem,
    Quit,
}

impl TryFrom<Option<usize>> for UserSelection {
    type Error = anyhow::Error;

    fn try_from(value: Option<usize>) -> Result<Self, Self::Error> {
        if let Some(selection) = value {
            match selection {
                0 => Ok(UserSelection::IncludeItem),
                1 => Ok(UserSelection::ExcludeItem),
                2 => Ok(UserSelection::IncludeAllItemsOfImplBlock),
                3 => Ok(UserSelection::ExcludeAllItemsOfImplBlock),
                4 => Ok(UserSelection::ShowItem),
                5 => Ok(UserSelection::ShowUsageOfItem),
                _ => Err(anyhow!(
                    "{}",
                    add_context!("Expected selection in range of UserSelection.")
                )),
            }
        } else {
            Ok(UserSelection::Quit)
        }
    }
}

#[automock]
pub trait CgDialog<S: Display + 'static, M: Display + 'static> {
    fn select_option(&self, prompt: &str, help: &str, options: Vec<S>) -> AnyResult<Option<S>>;
    fn text_file_path(
        &self,
        prompt: &str,
        help: &str,
        initial_value: &str,
    ) -> AnyResult<Option<Utf8PathBuf>>;
    fn write_output(&mut self, message: M) -> AnyResult<()>;
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
    fn select_option(&self, prompt: &str, help: &str, options: Vec<S>) -> AnyResult<Option<S>> {
        let selected_item = Select::new(prompt, options)
            .with_render_config(RenderConfig::default_colored())
            .with_help_message(help)
            .prompt_skippable()?;
        Ok(selected_item)
    }

    fn text_file_path(
        &self,
        prompt: &str,
        help: &str,
        initial_value: &str,
    ) -> AnyResult<Option<Utf8PathBuf>> {
        let file_path = Text::new(prompt)
            .with_render_config(RenderConfig::default_colored())
            .with_help_message(help)
            .with_initial_value(initial_value)
            .with_autocomplete(FilePathCompleter::default())
            //.with_validators(validators) ToDo: validate for now white space and file ending on .toml
            .prompt_skippable()?
            .map(|fp| Utf8PathBuf::from(fp));
        Ok(file_path)
    }

    fn write_output(&mut self, message: M) -> AnyResult<()> {
        write!(self.writer, "{}", message)?;
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct FilePathCompleter {
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
                if p.to_string() == "" {
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

        let entries = match fs::read_dir(scan_dir) {
            Ok(read_dir) => Ok(read_dir),
            Err(err) if err.kind() == ErrorKind::NotFound => fs::read_dir(fallback_parent),
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

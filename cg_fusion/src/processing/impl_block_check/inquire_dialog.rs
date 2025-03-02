// inquire dialog helper functions

use crate::{add_context, utilities::is_inside_dir};
use anyhow::anyhow;
use cargo_metadata::camino::Utf8PathBuf;
use inquire::{
    CustomUserError,
    validator::{ErrorMessage, StringValidator, Validation},
};
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq)]
pub enum DialogImplItemSelection {
    IncludeItem,
    ExcludeItem,
    IncludeAllItemsOfImplBlock,
    ExcludeAllItemsOfImplBlock,
    ShowItem,
    ShowUsageOfItem,
    Quit,
}

impl TryFrom<Option<usize>> for DialogImplItemSelection {
    type Error = anyhow::Error;

    fn try_from(value: Option<usize>) -> Result<Self, Self::Error> {
        if let Some(selection) = value {
            match selection {
                0 => Ok(DialogImplItemSelection::IncludeItem),
                1 => Ok(DialogImplItemSelection::ExcludeItem),
                2 => Ok(DialogImplItemSelection::IncludeAllItemsOfImplBlock),
                3 => Ok(DialogImplItemSelection::ExcludeAllItemsOfImplBlock),
                4 => Ok(DialogImplItemSelection::ShowItem),
                5 => Ok(DialogImplItemSelection::ShowUsageOfItem),
                _ => Err(anyhow!(
                    "{}",
                    add_context!("Expected selection in range of DialogImplItemSelection.")
                )),
            }
        } else {
            Ok(DialogImplItemSelection::Quit)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DialogImplBlockSelection {
    IncludeImplBlock,
    ExcludeImplBlock,
    IncludeAllItemsOfImplBlock,
    ShowImplBlock,
    Quit,
}

impl TryFrom<Option<usize>> for DialogImplBlockSelection {
    type Error = anyhow::Error;

    fn try_from(value: Option<usize>) -> Result<Self, Self::Error> {
        if let Some(selection) = value {
            match selection {
                0 => Ok(DialogImplBlockSelection::IncludeImplBlock),
                1 => Ok(DialogImplBlockSelection::ExcludeImplBlock),
                2 => Ok(DialogImplBlockSelection::IncludeAllItemsOfImplBlock),
                3 => Ok(DialogImplBlockSelection::ShowImplBlock),
                _ => Err(anyhow!(
                    "{}",
                    add_context!("Expected selection in range of DialogImplBlockSelection.")
                )),
            }
        } else {
            Ok(DialogImplBlockSelection::Quit)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DialogImplBlockWithTraitSelection {
    IncludeImplBlock,
    ExcludeImplBlock,
    ShowImplBlock,
    Quit,
}

impl TryFrom<Option<usize>> for DialogImplBlockWithTraitSelection {
    type Error = anyhow::Error;

    fn try_from(value: Option<usize>) -> Result<Self, Self::Error> {
        if let Some(selection) = value {
            match selection {
                0 => Ok(DialogImplBlockWithTraitSelection::IncludeImplBlock),
                1 => Ok(DialogImplBlockWithTraitSelection::ExcludeImplBlock),
                2 => Ok(DialogImplBlockWithTraitSelection::ShowImplBlock),
                _ => Err(anyhow!(
                    "{}",
                    add_context!(
                        "Expected selection in range of DialogImplBlockWithTraitSelection."
                    )
                )),
            }
        } else {
            Ok(DialogImplBlockWithTraitSelection::Quit)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigFilePathValidator {
    pub base_dir: Utf8PathBuf,
}

impl StringValidator for ConfigFilePathValidator {
    fn validate(&self, input: &str) -> Result<Validation, CustomUserError> {
        let input_path = Utf8PathBuf::from_str(input.trim());
        let input_path = input_path?;

        // validate extension is toml
        match input_path.extension() {
            Some(ex) => {
                if ex != "toml" {
                    return Ok(Validation::Invalid(ErrorMessage::Custom(
                        "Config file path must end on '.toml'.".into(),
                    )));
                }
            }
            None => {
                return Ok(Validation::Invalid(ErrorMessage::Custom(
                    "Config file path must end on '.toml' with non-empty filename.".into(),
                )));
            }
        }

        // validate filename
        match input_path.file_stem() {
            Some(name) => {
                if name.chars().any(char::is_whitespace) {
                    return Ok(Validation::Invalid(ErrorMessage::Custom(
                        "Config file name must not contain whitespace.".into(),
                    )));
                }
                if name
                    .chars()
                    .any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                {
                    return Ok(Validation::Invalid(ErrorMessage::Custom(
                        "Config file name must only contain alphanumeric letters or '-' or '_'."
                            .into(),
                    )));
                }
                if !name.chars().next().map_or(false, |c| c.is_alphanumeric())
                    || !name.chars().last().map_or(false, |c| c.is_alphanumeric())
                {
                    return Ok(Validation::Invalid(ErrorMessage::Custom(
                        "Config file name must start and end with alphanumeric letter.".into(),
                    )));
                }
            }
            None => {
                return Ok(Validation::Invalid(ErrorMessage::Custom(
                    "Config file name must not be empty.".into(),
                )));
            }
        }

        // validate path is inside base dir
        if !is_inside_dir(&self.base_dir, &input_path)? {
            return Ok(Validation::Invalid(ErrorMessage::Custom(format!(
                "Config file path must be inside challenge dir '{}'.",
                self.base_dir
            ))));
        }

        Ok(Validation::Valid)
    }
}

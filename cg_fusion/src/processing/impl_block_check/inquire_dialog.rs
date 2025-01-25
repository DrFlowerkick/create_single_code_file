// inquire dialog helper functions

use crate::{add_context, utilities::is_inside_dir};
use anyhow::anyhow;
use cargo_metadata::camino::Utf8PathBuf;
use inquire::{
    validator::{ErrorMessage, StringValidator, Validation},
    CustomUserError,
};
use std::str::FromStr;

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
                )))
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
                )))
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

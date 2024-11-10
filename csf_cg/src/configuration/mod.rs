// configuration options

mod make;

pub use make::CliMake;

use structopt::StructOpt;
use std::str::FromStr;
use std::fmt::{self, Display};
use crate::CGError;




#[derive(Debug, StructOpt)]
pub enum OutputMode {
    /// Merge input file with all of it's dependencies and create a new file or overwrite the existing output file
    Merge,

    /// Update the existing output file with the specified components
    Update,

    /// Create a new file with an incremented number at the end
    Increment,
}

impl FromStr for OutputMode {
    type Err = CGError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Merge" | "merge" => Ok(Self::Merge),
            "Update" | "update" => Ok(Self::Update),
            "Increment" | "increment" => Ok(Self::Increment),
            _ => Err(CGError::NotAcceptedOutputMode),
        }
    }
}

impl Display for OutputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputMode::Merge => write!(f, "Merge"),
            OutputMode::Update => write!(f, "Update"),
            OutputMode::Increment => write!(f, "Increment"),
        }
    }
}
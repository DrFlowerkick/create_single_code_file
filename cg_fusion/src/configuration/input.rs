// input options of cli

use clap::{Args, ValueEnum};
use std::fmt::{self, Display};
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
        writeln!(f, "platform: {}", self.platform)?;
        writeln!(
            f,
            "other-supported-crates: {:?}",
            self.other_supported_crates
        )
    }
}

#[cfg(test)]
impl Default for InputOptions {
    fn default() -> Self {
        Self {
            input: "main".into(),
            platform: ChallengePlatform::Codingame,
            other_supported_crates: Vec::new(),
        }
    }
}

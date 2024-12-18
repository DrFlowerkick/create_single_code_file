use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use structopt::StructOpt;

pub type BoxResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "Create_single_file_codingame_CLI",
    about = "Command Line Options for create_codingame_single_file",
    no_version,
    author = "Dr. Flowerkick"
)]
pub struct Cli {
    /// Load main.rs from path
    #[structopt(parse(from_os_str))]
    pub input: PathBuf,

    /// Output file, stdout if not present
    #[structopt(parse(from_os_str))]
    pub output: Option<PathBuf>,

    /// Only rebuild challenge code. Requires already existing output file with correct markers for main.rs and possible locale modules
    #[structopt(short, long)]
    pub challenge_only: bool,

    /// Select specific modules separated by ";" from local lib (default "all"). Module name is same name you use with "use" command in Rust.
    /// keyword "all": select all modules required by main.rs
    /// keyword "lib": select all required modules in crate of main.rs
    #[structopt(short, long, default_value = "all")]
    pub modules: String,

    /// block specific hidden modules seperated by ";" from lib (default ""). Module name is same name you use with "use" command in Rust.
    #[structopt(short, long, default_value = "")]
    pub block_hidden: String,

    /// name of local lib with locale modules. Same name as used in toml file.
    #[structopt(short, long, default_value = "my_lib")]
    pub lib: String,

    /// print verbose for self checking
    #[structopt(short, long)]
    pub verbose: bool,

    /// simulate execution without creating output; this option automatically set's verbose option
    #[structopt(short, long)]
    pub simulate: bool,

    /// delete comments
    #[structopt(short, long)]
    pub del_comments: bool,

    /// do not delete empty lines
    #[structopt(short, long)]
    pub keep_empty_lines: bool,
}

impl fmt::Display for Cli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "input: {:?}", self.input)?;
        writeln!(f, "output: {:?}", self.output)?;
        writeln!(f, "challenge_only: {}", self.challenge_only)?;
        writeln!(f, "modules: {}", self.modules)?;
        writeln!(f, "block_hidden: {}", self.block_hidden)?;
        writeln!(f, "lib: {}", self.lib)?;
        writeln!(f, "verbose: {}", self.verbose)?;
        writeln!(f, "simulate: {}", self.simulate)
    }
}

#[derive(Debug)]
pub enum CGError {
    MustProvideInPutFile,
    MustProvideOutPutFile,
    PackageStructureError(PathBuf),
    OutputFileError(PathBuf),
    NoStartLine(usize),
    NoEndLine,
    TooManyClosingBrackets,
    CouldNotFindEnumName,
}

impl fmt::Display for CGError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MustProvideInPutFile => {
                write!(f, "No input file main.rs specified!")
            }
            Self::MustProvideOutPutFile => {
                write!(f, "No output file specified with active insert options!")
            }
            Self::PackageStructureError(path) => write!(
                f,
                "input path \"{:?}\" does not fit to crate package structure",
                path
            ),
            Self::OutputFileError(path) => write!(
                f,
                "output path \"{:?}\" does not point to /src/bin dir in crate directory",
                path
            ),
            Self::NoStartLine(message_line) => write!(
                f,
                "Could not find start line of name space for message line {}",
                message_line
            ),
            Self::NoEndLine => write!(f, "Could not find end line of name space"),
            Self::TooManyClosingBrackets => write!(
                f,
                "More closing brackets than starting brackets for name space"
            ),
            Self::CouldNotFindEnumName => {
                write!(f, "Could not find enum name of never constructed variant")
            }
        }
    }
}

impl Error for CGError {}

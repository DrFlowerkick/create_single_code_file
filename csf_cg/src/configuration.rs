use std::error::Error;
use structopt::StructOpt;
use std::path::PathBuf;
use std::fmt;

pub type BoxResult<T> = Result<T,Box<dyn Error>>;

#[derive(Debug, StructOpt)]
#[structopt(name = "Create_single_file_codingame_CLI",
           about = "Command Line Options for create_codingame_single_file",
           no_version,
           author = "Dr. Flowerkick")]
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

    /// Select specific modules separated by ";" from lib or all (default all). module name is same name you use with "use" command in Rust 
    #[structopt(short, long, default_value = "all")]
    pub modules: String,

    /// block specific hidden modules seperated by ";" from lib (default none). module name is same name you use with "use" command in Rust 
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

    /// delete comment
    #[structopt(short, long)]
    pub del_comments: bool,
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
    MustProvideOutPutFile,
    PackageStructureError(PathBuf),
    NoStartLine(usize),
    NoEndLine,
    TooManyClosingBrackets,
}

impl fmt::Display for CGError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MustProvideOutPutFile => write!(f, "No output file specified with active insert options!"),
            Self::PackageStructureError(path) => write!(f, "input path \"{:?}\" does not fit to crate package structure", path),
            Self::NoStartLine(message_line) => write!(f, "Could not find start line of name space for message line {}", message_line),
            Self::NoEndLine => write!(f, "Could not find end line of name space"),
            Self::TooManyClosingBrackets => write!(f, "More closing brackets than starting brackets for name space"),
        }
    }
}

impl Error for CGError {}

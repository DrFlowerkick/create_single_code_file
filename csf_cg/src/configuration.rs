// configuration options

use super::{CGData, preparation::PrepState, error::CGError};

use std::fmt::{self, Display};
use std::path::PathBuf;
use std::collections::BTreeMap;
use std::str::FromStr;
use structopt::StructOpt;

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

#[derive(Debug, StructOpt)]
#[structopt(
    name = env!("CARGO_PKG_NAME"),
    about = "Command Line Options for create_codingame_single_file",
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
    help = "This tool is used for codingame and similar challenges. codingame requires a \n\
            single source file containing all required code. It supports the std library\n\
            and some crates from crates.io (e.g. rand). Since sane programmers use code\n\
            libraries for reusable and modular code, the requirements of codingame are\n\
            counter intuitive for good practices. To participate at codingame and use\n\
            a local code library all required src files have to be merged in one clumpy\n\
            file and than purged from unused dead library code. Here comes this tool into\n\
            play by doing exactly that.\n\n\
            All work of the tool is done inside a temporary directory, which is created in\n\
            the directory which contains the challenge crate directory. The name of the\n\
            temporary directory is a uuid. After all work is done, the generated output\n\
            file will be copied to it's configured destination and the temporary directory\n\
            will be deleted.\n\n\
            In 'analysis_only' mode the tool shows all dependencies of the input file to\n\
            to crate modules and local library modules. It also shows a warning, if the\n\
            local library has dependencies to crate.io, which are not fulfilled by the\n\
            input crate. In this case you either have to add these dependencies to the\n\
            input crate toml (if the are required by the challenge code) or use the '--force'\n\
            option to merge and filter the output without the crate.io dependencies.",
)]
pub struct Cli {
    /// Load main.rs from path; error if not main.rs
    #[structopt(parse(from_os_str), help = "Path of to be processed binary crate main file.")]
    pub input: PathBuf,

    /// filename of new bin; must include rs extension and must not be main.rs
    /// Output will be saved in ./src/bin of input crate
    #[structopt(
        help = "Filename of merged src file.",
        long_help = "Filename of merged src file without rs extension.\n\
                     Output file will be saved in ./src/bin/filename.rs of input crate.\n\
                     Resulting output file must be different from 'input'.\n\
                     'output_mode' specifies how to handle file output.\n\
                      a temporary working directory with the content of\n\
                     input crate directory. File handling and filtering of sequences hinted\n\
                     at by compiler messages are done in temporary directory.\n\
                     The temporary directory is created in the directory which contains the\n\
                     input crate directory. The name of the directory is a uuid.",
    )]
    pub filename: String,

    /// Mode for handling the output file: 'Merge', 'Update', or 'Increment'
    #[structopt(
        short,
        long,
        default_value = "Merge",
        help = "Specifies how to handle file output. Default is overwrite mode.",
        long_help = "Determines how the output file should be handled:\n\
                     - Merge (default mode): Merge input file with all of it's dependencies\n\
                     and create a new file or overwrite the existing output file.\n\
                     - Update: Updates existing file with configured components. Falls back\n\
                     to Overwrite if no file exists.\n\
                     - Increment: If output file does not exist, this mode works like Merge,\n\
                     but sets output filename to filename.001.rs. If output file(s) do exist,\n\
                     this mode works like Update on the newest existing temporary output file\n\
                     and saves changes in a new file by incrementing the filename number.\n\
                     This mode is very useful to comprehend merging and filtering of src files.\n\
                     Use 'max_filter_cycles' and 'max_steps_per_cycle' for fine control.",
    )]
    pub output_mode: OutputMode,

    /// Name of local lib with locale modules. Same name as used in toml file.
    #[structopt(
        short,
        long,
        default_value = "my_lib",
        help = "Name of local library crate used in input crate, if any is used. Same name as specified in toml file.",
    )]
    pub lib: String,

    /// Analyze input for dependencies and blocked hidden modules.
    /// Does not create output or filter input.
    /// This option automatically set's verbose option
    #[structopt(
        short,
        long,
        help = "Analyze input for dependencies and blocked hidden modules.",
        long_help = "Analyze input for dependencies and blocked hidden modules.\n\
                     Does not create output or filter input.\n\
                     This option automatically set's verbose option.",
    )]
    pub analysis_only: bool,

    /// Only merge output, do not filter.
    #[structopt(
        short,
        long,
        help = "Only merges main.rs with input crate files and local dependencies of library without\n\
                filtering spans reported by 'cargo check'.",
    )]
    pub merging_only: bool,

    /// Only filtering of output, no merging. Requires already existing output file.
    #[structopt(
        short,
        long,
        help = "Only filters spans reported by 'cargo check'.",
        long_help = "Only filters spans by 'cargo check'. Does not merge main.rs with input\n\
                     crate files and local dependencies of library.\n\
                     Requires already existing output file.",
    )]
    pub filtering_only: bool,

    /// Max number of filter cycles. Each cycles handles a set of compiler errors or warnings step by step from bottom to top.
    /// Min number is always 1.
    #[structopt(
        short = "y",
        long,
        default_value = "1000",
        help = "Sets max number of filter cycles of compiler messages (default 1000).",
        long_help = "Filtering of sequences hinted at by compiler messages works in cycles.\n\
                     Each cycle runs 'cargo check' on current temporary directory.\n\
                     First it checks for Errors. If no Errors exist, it checks for Warnings.\n\
                     If no Warnings exist, filtering is finished.\n\
                     All entries are saved in a BTreeMap and than resolved step by step from\n\
                     bottom to top of temporary output file. Afterward a new cycle starts.\n\
                     This option sets the max number of cycles (default 1000). Minimum is one cycle.",
    )]
    pub max_filter_cycles: usize,

    /// Max number of filter steps per cycle. Each step handles a compiler error or warning from bottom to top.
    /// Min number is always 1.
    /// Stops cycle and filtering if max steps are reached
    #[structopt(
        short = "x",
        long,
        default_value = "1000",
        help = "Sets max number of steps for a filter cycle of compiler messages (default 1000).",
        long_help = "Each filter cycle consists of a number of compiler messages sorted by line of\n\
                     occurrence. Each compiler message counts as one step. The steps are resolved\n\
                     from bottom to top of of temporary output file. If the maximum number of steps\n\
                     is reached, filtering is stopped and current output is saved.\n\
                     This option sets the max number of steps (default 1000). Minimum is one step.",
    )]
    pub max_steps_per_cycle: usize,

    /// Select specific components separated by ";" to update, if output_mode is Update
    /// or Increment. Use main for main input file and module names for specific modules.
    /// Namespace path of module is only required bijective names must be ensured.
    /// Instead of a list two keywords are supported:
    /// "crate": select main.rs and all modules of input crate (default value).
    /// "lib": select all required modules of library crate.
    #[structopt(
        short,
        long,
        default_value = "crate",
        help = "List of components to update, if output_mode is Update or Increment. Entries are seperated by ';'.",
        long_help = "Select specific components separated by ';' to update, if output_mode is Update\n\
                     or Increment. Use main for main input file and module names for specific modules.\n
                     Namespace path of module is only required bijective names must be ensured.\n\
                     Instead of a list two keywords are supported:\n\
                     'crate': select main.rs and all modules of input crate (default value).\n\
                     'lib': select all required modules of library crate.",
    )]
    pub update_components: String,

    /// Block specific hidden modules seperated by ";" from lib (default "").
    /// Module name is same name you use with "use" command in Rust.
    /// Namespace path of module is only required bijective names must be ensured.
    #[structopt(
        short,
        long,
        default_value = "",
        help = "List of hidden modules to block from processing. Entries are seperated by ';'.",
        long_help = "Library crates contain a lot of functions and modules can depend upon further\n\
                     modules inside the library. If these modules are not referenced with a use statement\n\
                     inside a crate src file, they are called hidden modules. Some of these hidden modules\n
                     are not required of the input crate. 'block_hidden' is used to block hidden modules\n\
                     from processing to speed up execution time. Namespace path of module is only required\n\
                     bijective names must be ensured.",
    )]
    pub block_hidden: String,

    /// Force merging and filtering if lib dependencies on crate.io are not fulfilled by input crate.
    #[structopt(
        long,
        help = "Force merging and filtering if lib dependencies on crate.io are not fulfilled by input crate.",
    )]
    pub force: bool,

    /// Print extended information during execution
    #[structopt(
        short,
        long,
        help = "Print extended information during execution.",
    )]
    pub verbose: bool,

    /// Delete comments
    #[structopt(
        short,
        long,
        help = "Delete comments from output.",
    )]
    pub del_comments: bool,

    /// Do not delete empty lines
    #[structopt(
        short,
        long,
        help = "Keep empty lines in output.",
    )]
    pub keep_empty_lines: bool,
}

impl fmt::Display for Cli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "input: {:?}", self.input)?;
        writeln!(f, "filename: {:?}", self.filename)?;
        writeln!(f, "output_mode: {}", self.output_mode)?;
        writeln!(f, "lib: {}", self.lib)?;
        writeln!(f, "analysis_only: {}", self.analysis_only)?;
        writeln!(f, "merging_only: {}", self.merging_only)?;
        writeln!(f, "filtering_only: {}", self.filtering_only)?;
        writeln!(f, "max_filter_cycles: {}", self.max_filter_cycles)?;
        writeln!(f, "max_steps_per_cycle: {}", self.max_steps_per_cycle)?;
        writeln!(f, "update_components: {}", self.update_components)?;
        writeln!(f, "block_hidden: {}", self.block_hidden)?;
        writeln!(f, "force: {}", self.force)?;
        writeln!(f, "verbose: {}", self.verbose)?;
        writeln!(f, "del_comments: {}", self.del_comments)?;
        writeln!(f, "keep_empty_lines: {}", self.keep_empty_lines)
    }
}

impl Cli {
    pub fn initialize_cg_data(self) -> CGData<PrepState> {
        let mut result: CGData<PrepState> = CGData {
            options: self,
            state_data: PrepState {  },
            crate_dir: PathBuf::new(),
            crate_name: "".to_string(),
            local_modules: BTreeMap::new(),
            my_lib: None,
            lib_modules: BTreeMap::new(),
            tmp_dir: PathBuf::new(),
            tmp_input_file: PathBuf::new(),
            tmp_output_file: PathBuf::new(),
            output_file: PathBuf::new(),
            line_end_chars: "".to_string(),
        };
        if result.options.analysis_only {
            result.options.verbose = true;
        }
        if result.options.verbose {
            println!("{}", result.options);
        }
        result
    }
}
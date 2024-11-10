/// Configuration options of cargo-cg_make

use crate::{CGData, preparation::PrepState};
use super::OutputMode;

use std::fmt::{self, Display};
use std::path::PathBuf;
use std::collections::BTreeMap;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cg_make",
    about = "Command Line Options for create_codingame_single_file",
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
    help = "cg_make is a handy extension of cargo for codingame and similar online\n\
            challenges, which require a single source file containing all required code.\n\
            codingame supports the rust std library and some crates from crates.io (e.g. rand).\n\
            Since sane programmers use code libraries for reusable and modular code, the\n\
            requirements of codingame are counter intuitive for good practices. To participate\n\
            at codingame and use a local code library all required src files have to be merged\n\
            in one clumpy file and than purged from unused dead library code. Here comes \n\
            cg_make into play by doing exactly that.\n\n\
            Run cg_make with 'cargo cg_make' in the root directory of your challenge crate (as\n\
            is normal for all cargo commands). By default cg_make takes 'main.rs' and analyzes\n\
            it for all dependencies inside the crate and a local library, if applicable. If there\n\
            are no dependencies cg_make does nothing. Otherwise cg_make will merge all required\n\
            files into one single src file and saves it by default as 'challenge_name_of_package.rs'\n\
            inside of 'challenge_crate_dir/src/bin/'. Normally this merged file contains\n\
            unwanted code fragments, which either prevent it from compilation or is dead code,\n\
            which just takes up space. Therefore the next step of cg_make is to call repeatedly\n\
            'cargo check' and purging step by step unwanted code fragments. This process is\n\
            unstable and may result in in broken code. Therefore cg_make creates always a temporary\n\
            file inside of 'crate_dir/src/bin/' to work with. If no error occurs, this temporary\n\
            file is copied to the target location and than deleted. There are some options to\n\
            control this behavior. There are also stand alone cargo extensions to execute these\n\
            phases for more fine control of the process.\n\n\
            One warning about crates.io dependencies: cg_make does not pull these dependencies\n\
            and merge them into the challenge file. If the challenge code does depend upon other\n\
            crates than 'rand', the challenge code my not work on codingame. If the local library\n\
            has dependencies to crate.io, which are not fulfilled by the challenge crate, cg_make\n\
            shows a warning and does not process the challenge code. In this case either add these\n\
            dependencies to the challenge crate (if the are required by the challenge code) or use\n\
            'cargo cg_make --force' to proceed.",
)]
pub struct CliMake {
    /// Filename of input binary without rs extension (default is 'main')
    #[structopt(
        short,
        long,
        default_value = "main",
        help = "Path of to be processed binary crate main file.",
    )]
    pub input: String,

    /// Filename of to be created challenge binary. Default is 'challenge_name_of_challenge_crate'
    /// File will be saved in ./src/bin of challenge crate
    #[structopt(
        short,
        long,
        help = "Filename of challenge src file.",
        long_help = "Filename of challenge src file without rs extension.\n\
                     Default is 'challenge_name_of_challenge_crate'. Challenge src file\n\
                     will be saved in ./src/bin/ of challenge crate.\n\
                     'output_mode' specifies how to handle file output.",
    )]
    pub filename: Option<String>,

    /// Mode for handling the output file: 'Merge', 'Update', or 'Increment'
    #[structopt(
        short = "n",
        long,
        default_value = "Merge",
        help = "Specifies how to handle file output. Default is Merge mode.",
        long_help = "Determines how the output file should be handled:\n\
                     - Merge (default mode): Merge challenge with all of it's dependencies\n\
                     and create a new file or overwrite the existing output file.\n\
                     - Update: Updates existing file with configured components. Falls back\n\
                     to Merge if no file exists.\n\
                     - Increment: If output file does not exist, this mode works like Merge,\n\
                     but sets output filename to filename.001.rs. If output file(s) do exist,\n\
                     this mode works like Update on the newest existing temporary output file\n\
                     and saves changes in a new file by incrementing the filename number.\n\
                     This mode is very useful to comprehend merging and filtering of src files.\n\
                     Use 'max_filter_cycles' and 'max_steps_per_cycle' for fine control.",
    )]
    pub output_mode: OutputMode,

    /// Name of local lib with local modules. Same name as used in toml file.
    #[structopt(
        short,
        long,
        default_value = "my_lib",
        help = "Name of local library crate used in challenge crate, if any is used. Same name as specified in toml file.",
    )]
    pub lib: String,

    /// Analyze challenge for dependencies and blocked hidden modules.
    /// Does not create a merged challenge file.
    /// This option automatically set's verbose option
    #[structopt(
        short,
        long,
        help = "Analyze challenge for dependencies and blocked hidden modules.",
        long_help = "Analyze challenge for dependencies and blocked hidden modules.\n\
                     Does not create a merged challenge file.\n\
                     This option automatically set's verbose option.",
    )]
    pub analyze_only: bool,

    /// Only merge output, do not check and fix it.
    #[structopt(
        short,
        long,
        help = "Only merges challenge with dependencies of without removing\n\
                problematic code spans reported by 'cargo check'.",
    )]
    pub merge_only: bool,

    /// Only purging of problematic code spans without merging of challenge code. Requires already existing output file.
    #[structopt(
        short,
        long,
        help = "Only purges problematic code spans reported by 'cargo check'.",
        long_help = "Only purges problematic code spans reported by 'cargo check'.\n\
                     Does not merge challenge with dependencies.\n\
                     Requires already existing output file.",
    )]
    pub purge_only: bool,

    /// Max number of purge cycles. Each cycles handles a set of compiler errors or warnings step by step from bottom to top.
    /// Min number is always 1.
    #[structopt(
        short = "y",
        long,
        default_value = "1000",
        help = "Sets max number of purge cycles of compiler messages (default 1000).",
        long_help = "Purging of of code spans reported by compiler messages works in cycles.\n\
                     Each cycle runs 'cargo check' on crate directory, therefore checking the newly\n\
                     created binary of merged challenge code.\n\
                     First it checks for Errors. If no Errors exist, it checks for Warnings.\n\
                     If no Warnings exist, purging is finished.\n\
                     All entries are saved in a BTreeMap and than resolved step by step from\n\
                     bottom to top of temporary output file. Afterward a new cycle starts.\n\
                     This option sets the max number of cycles (default 1000). Minimum is one cycle.",
    )]
    pub max_purge_cycles: usize,

    /// Max number of purge steps per cycle. Each step handles a compiler error or warning from bottom to top.
    /// Min number is always 1.
    /// Stops cycle and purging if max steps are reached
    #[structopt(
        short = "x",
        long,
        default_value = "1000",
        help = "Sets max number of steps for a purge cycle of compiler messages (default 1000).",
        long_help = "Each purge cycle consists of a number of compiler messages sorted by line of\n\
                     occurrence. Each compiler message counts as one step. The steps are resolved\n\
                     from bottom to top of of temporary output file. If the maximum number of steps\n\
                     is reached, purging is stopped and current output is saved.\n\
                     This option sets the max number of steps (default 1000). Minimum is one step.",
    )]
    pub max_steps_per_cycle: usize,

    /// Select specific components separated by ";" to update, if output_mode is Update
    /// or Increment. Use main for main input file and module names for specific modules.
    /// Namespace path of module is only required bijective names must be ensured.
    /// Instead of a list two keywords are supported:
    /// "crate": select src files of challenge crate (default value).
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
                     'crate': select src files of challenge crate (default value).\n\
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

    /// Force merging and purging if lib dependencies on crate.io are not fulfilled by challenge crate.
    #[structopt(
        short = "f",
        long,
        help = "Force merging and purging if lib dependencies on crate.io are not fulfilled by challenge crate.",
    )]
    pub force: bool,

    /// Print extended information during execution
    #[structopt(
        short,
        long,
        help = "Print extended information during execution.",
    )]
    pub verbose: bool,

    /// Keep comments
    #[structopt(
        short = "c",
        long,
        help = "Keep comments in merge challenge file.",
    )]
    pub keep_comments: bool,

    /// Do not delete empty lines
    #[structopt(
        short = "e",
        long,
        help = "Keep empty lines in merged challenge file.",
    )]
    pub keep_empty_lines: bool,

    /// Do not delete temporary file if error occurred
    #[structopt(
        short = "t",
        long,
        help = "Keep temporary file if error occurred."
    )]
    pub keep_tmp_file: bool,
}

impl Display for CliMake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "input: {}", self.input)?;
        writeln!(f, "filename: {:?}", self.filename)?;
        writeln!(f, "output_mode: {}", self.output_mode)?;
        writeln!(f, "lib: {}", self.lib)?;
        writeln!(f, "analyze_only: {}", self.analyze_only)?;
        writeln!(f, "merge_only: {}", self.merge_only)?;
        writeln!(f, "purge_only: {}", self.purge_only)?;
        writeln!(f, "max_purge_cycles: {}", self.max_purge_cycles)?;
        writeln!(f, "max_steps_per_cycle: {}", self.max_steps_per_cycle)?;
        writeln!(f, "update_components: {}", self.update_components)?;
        writeln!(f, "block_hidden: {}", self.block_hidden)?;
        writeln!(f, "force: {}", self.force)?;
        writeln!(f, "verbose: {}", self.verbose)?;
        writeln!(f, "keep_comments: {}", self.keep_comments)?;
        writeln!(f, "keep_empty_lines: {}", self.keep_empty_lines)?;
        writeln!(f, "keep_tmp_file: {}", self.keep_tmp_file)
    }
}

impl CliMake {
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
        if result.options.analyze_only {
            result.options.verbose = true;
        }
        if result.options.verbose {
            println!("{}", result.options);
        }
        result
    }
}
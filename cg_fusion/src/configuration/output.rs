// output options of cli

use clap::Args;
use std::fmt::{self, Display};

#[derive(Debug, Args)]
pub struct OutputOptions {
    /// Filename of merged challenge src file without rs extension. Default filename
    /// is 'fusion_of_name_of_challenge_crate'. Challenge src file will be saved
    /// in './src/bin/' of the challenge crate. If this output file already exists
    /// you must use '-f' or '--force' to overwrite it.
    #[arg(
        short = 'n',
        long,
        help = "Filename of merged challenge src file without rs extension."
    )]
    pub filename: Option<String>,

    /// In debug mode temporary files are used for initial merged output file and for each
    /// purge cycle. Analyze these files if you get unexpected results. If no error occurs,
    /// these temporary files will be deleted.
    #[arg(short, long, help = "Activate debug mode.")]
    pub debug: bool,

    /// Do not delete temporary fusion files, even if no error occurs.
    #[arg(
        short = 't',
        long,
        requires = "debug",
        help = "Do not delete temporary fusion files, even if no error occurs. Requires debug mode."
    )]
    pub keep_tmp_file: bool,
}

impl Display for OutputOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "filename: {:?}", self.filename)?;
        writeln!(f, "debug: {}", self.debug)?;
        writeln!(f, "keep-tmp-file: {}", self.keep_tmp_file)
    }
}

// Stuff required for CLI parsing
use structopt::StructOpt;

use create_codingame_single_file::configuration::*;
use create_codingame_single_file::*;

fn main() {
    let options = Cli::from_args();
    if let Err(err) = run(options) {
        eprintln!("Error occurred: {}", err);

        // look for source
        if let Some(source) = err.source() {
            eprintln!("Source of error: {:?}", source);
        }
    }
}

fn run(options: Cli) -> BoxResult<()> {
    let mut data = CGData::new(options);
    data.prepare_cg_data()?;
    data.create_output()?;
    data.filter_unused_code()?;
    data.cleanup_cg_data()?;
    Ok(())
}

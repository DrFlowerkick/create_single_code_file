// Stuff required for CLI parsing
use structopt::StructOpt;

use create_codingame_single_file::configuration::*;
use create_codingame_single_file::error::CGResult;

fn main() {
    let options = Cli::from_args();
    if let Err(err) = run(options) {
        eprintln!("Error occurred: {:?}", err);
    }
}

fn run(options: Cli) -> CGResult<()> {
    let mut data = options.initialize_cg_data();
    //data.prepare_cg_data()?;
    //data.create_output()?;
    //data.filter_unused_code()?;
    //data.cleanup_cg_data()?;
    Ok(())
}

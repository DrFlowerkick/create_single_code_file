// Stuff required for CLI parsing
use structopt::StructOpt;

use create_codingame_single_file::configuration::*;
use create_codingame_single_file::*;

fn main() {
    let options = Cli::from_args();
    if let Err(err) = run(options) {
        println!("Error occured: {}", err);
        
        // look for source
        match err.source() {
            Some(source) => println!("Source of error: {:?}", source),
            None => (),
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



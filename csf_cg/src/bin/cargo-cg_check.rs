/// cargo-cg_check takes a single challenge file and solves all 'cargo check'
/// messages by removing dead code. Has to be executed in root folder of a
/// package crate. Does not support workspace crates.
/// cargo-cg_check is a "short-cut" binary for configuration 'check_only' of
/// cargo-cg_make.

use anyhow::Context;
// Stuff required for CLI parsing
use structopt::StructOpt;

use create_codingame_challenge_file::configuration::*;
use create_codingame_challenge_file::error::CGResult;

fn main() {
    let options = Cli::from_args();
    if let Err(err) = run(options) {
        eprintln!("Error occurred: {:?}", err);
    }
}

fn run(_options: Cli) -> CGResult<()> {
    //let mut data = options.initialize_cg_data();
    //data.prepare_cg_data()?;
    //data.create_output()?;
    //data.filter_unused_code()?;
    //data.cleanup_cg_data()?;

    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path("./Cargo.toml")
        .exec()
        .context("Test of cargo metadata")?;

    println!("{:?}", metadata.root_package());

    Ok(())
}
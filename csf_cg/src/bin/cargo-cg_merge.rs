/// cargo-cg_merge takes main.rs of crate and merges it with all dependencies
/// into a single challenge file. Has to be executed in root folder of a
/// package crate. Does not support workspace crates.
/// cargo-cg_merge is a "short-cut" binary for configuration 'merge_only' of
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

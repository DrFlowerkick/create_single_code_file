/// cargo-cg_analyze takes main.rs of crate and analyses it for local dependencies:
///   - crate modules,
///   - local library if applicable.
/// Results are printed to std-out. No challenge file is created.
/// Has to be executed in root folder of a package crate. Does not support workspace crates.
/// cargo-cg_analyze is a "short-cut" binary for configuration 'analysis_only' of
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

    println!("{:?}", metadata.workspace_packages());

    Ok(())
}

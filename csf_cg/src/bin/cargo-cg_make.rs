/// cargo-cg_make takes main.rs of crate and merges it with all dependencies
/// into a single challenge file. Afterward solves all 'cargo check' messages
/// by removing dead code. Has to be executed in root folder of a package
/// crate. Does not support workspace crates.
/// cargo-cg_analyze, cargo-cg_merge, and cargo-cg_check are "short-cut"
/// binaries for configuration options cargo-cg_make.
/// After installation all commands can be run as cargo sub-command, e.g.
/// cargo cg_make.

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

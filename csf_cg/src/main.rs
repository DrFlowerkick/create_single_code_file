/// cargo-cg-fusion takes main.rs of crate and merges it with all dependencies
/// into a single challenge file. Afterward it solves all 'cargo check' messages
/// by purging dead code from the file. It has either to be executed in root folder
/// of a challenge crate or the manifest-path of the challenge crate has to be provided.
/// Does not support workspace crates.
/// cargo-cg-fusion acts as a cargo extension, as the name already suggests. It provides
/// three additional modes for fine control of fusion process: analyze, merge, and purge.
use anyhow::Context;

use cargo_cg_fusion::configuration::{CargoCli, CliCommon};
use cargo_cg_fusion::error::CGResult;
use clap::Parser;

fn main() {
    let options = CargoCli::parse();
    let delete_tmp_file = options.keep_tmp_files();
    if let Err(err) = run(options) {
        if let Some(true) = delete_tmp_file {
            // ToDo: check for tmp file(s), which have a valid uuid as filename and delete it.
        }
        eprintln!("Error occurred: {:?}", err);
    }
}

fn run(options: CargoCli) -> CGResult<()> {
    if let CargoCli::CgFusion(fusion_cli) = options {
        fusion_cli.verbose();
        let metadata = fusion_cli
            .manifest_command()
            .exec()
            .context("Test of cargo metadata")?;

        println!("{:?}", metadata.root_package());
    }
    //let _data = options.initialize_cg_data();
    //data.prepare_cg_data()?;
    //data.create_output()?;
    //data.filter_unused_code()?;
    //data.cleanup_cg_data()?;

    Ok(())
}
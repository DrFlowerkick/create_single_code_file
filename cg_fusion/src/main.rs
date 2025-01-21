/// cargo-cg-fusion takes main.rs of crate and merges it with all dependencies
/// into a single challenge file. Afterward it solves all 'cargo check' messages
/// by purging dead code from the file. It has either to be executed in root folder
/// of a challenge crate or the manifest-path of the challenge crate has to be provided.
/// Does not support workspace crates.
/// cargo-cg-fusion acts as a cargo extension, as the name already suggests. It provides
/// three additional modes for fine control of fusion process: analyze, merge, and purge.
use cargo_cg_fusion::{configuration::CargoCli, error::CgResult, CgDataBuilder, CgMode};
use clap::Parser;

fn main() {
    let options = CargoCli::parse();
    if let Err(err) = run(options) {
        eprintln!("Error occurred: {:?}", err);
    }
}

fn run(options: CargoCli) -> CgResult<()> {
    match CgDataBuilder::new()
        .set_options(options)
        .set_command()
        .build()?
    {
        CgMode::Fusion(fusion) => {
            fusion
                .add_challenge_dependencies()?
                .add_src_files()?
                .expand_use_statements()?
                .link_impl_blocks_with_corresponding_item()?
                .link_required_by_challenge()?
                .check_impl_blocks_required_by_challenge()?;
        }
    }
    Ok(())
}

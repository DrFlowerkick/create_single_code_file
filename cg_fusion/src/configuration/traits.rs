// trait definitions of cli options

use super::{InputOptions, OutputOptions};

pub trait CgCli {
    fn verbose(&self) -> bool;
    fn manifest_metadata_command(&self) -> cargo_metadata::MetadataCommand;
    fn force(&self) -> bool;
    fn input(&self) -> &InputOptions;
    fn output(&self) -> &OutputOptions;
}

pub trait CgCliImplDialog: CgCli {}

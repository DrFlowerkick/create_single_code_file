// trait definitions of cli options

use super::{InputOptions, MergeOptions, OutputOptions, PurgeOptions};

pub trait CliCommon {
    fn verbose(&self) -> bool;
    fn manifest_metadata_command(&self) -> cargo_metadata::MetadataCommand;
}

pub trait CliInput: CliCommon {
    fn input(&self) -> &InputOptions;
}

pub trait CliOutput: CliCommon {
    fn output(&self) -> &OutputOptions;
}

pub trait CliMerge: CliInput + CliOutput {
    fn merge(&self) -> &MergeOptions;
}

pub trait CliPurge: CliOutput {
    fn purge(&self) -> &PurgeOptions;
}

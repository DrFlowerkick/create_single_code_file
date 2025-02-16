// working with metadata

mod error;
use crate::add_context;
pub use error::{MetadataError, MetadataResult};

use anyhow::Context;
use cargo_metadata::{camino::Utf8PathBuf, Message, Metadata, MetadataCommand, Target};
use std::fmt::Write;
use std::{
    ops::Deref,
    process::{Command, Output},
};

#[derive(Debug, Clone)]
pub struct MetaWrapper(Metadata);

// try from path of Cargo.toml file
impl TryFrom<&Utf8PathBuf> for MetaWrapper {
    type Error = MetadataError;
    fn try_from(value: &Utf8PathBuf) -> Result<Self, Self::Error> {
        let metadata = cargo_metadata::MetadataCommand::new()
            .manifest_path(value)
            .exec()?;
        Ok(MetaWrapper(metadata))
    }
}

impl TryFrom<MetadataCommand> for MetaWrapper {
    type Error = MetadataError;
    fn try_from(value: MetadataCommand) -> Result<Self, Self::Error> {
        let metadata = value.exec()?;
        Ok(MetaWrapper(metadata))
    }
}

impl Deref for MetaWrapper {
    type Target = cargo_metadata::Metadata;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MetaWrapper {
    pub fn new(metadata: cargo_metadata::Metadata) -> Self {
        Self(metadata)
    }
    pub fn root_package(&self) -> MetadataResult<&cargo_metadata::Package> {
        self.0
            .root_package()
            .ok_or_else(|| MetadataError::NoRootPackage)
    }

    pub fn package_name(&self) -> MetadataResult<&str> {
        Ok(self.root_package()?.name.as_str())
    }

    pub fn package_manifest(&self) -> MetadataResult<&Utf8PathBuf> {
        Ok(&self.root_package()?.manifest_path)
    }

    pub fn package_root_dir(&self) -> MetadataResult<Utf8PathBuf> {
        let manifest_path = &self.root_package()?.manifest_path;
        Ok(manifest_path
            .parent()
            .ok_or_else(|| MetadataError::ErrorManifestPathOfMetadata(manifest_path.to_owned()))?
            .to_owned())
    }

    pub fn get_binary_target_of_root_package(&self, bin_name: &str) -> MetadataResult<&Target> {
        self.root_package()?
            .targets
            .iter()
            .find(|t| t.is_bin() && t.name == bin_name)
            .ok_or_else(|| MetadataError::BinaryNotFound(bin_name.to_owned()))
    }

    pub fn get_library_target_of_root_package(&self) -> MetadataResult<Option<&Target>> {
        Ok(self.root_package()?.targets.iter().find(|t| t.is_lib()))
    }

    pub fn get_member_manifests_of_workspace(&self) -> Vec<(String, Utf8PathBuf)> {
        self.0
            .workspace_members
            .iter()
            .filter_map(|pid| self.0.packages.iter().find(|p| p.id == *pid))
            .map(|p| (p.name.to_owned(), p.manifest_path.to_owned()))
            .collect()
    }
    fn run_cargo_command_for_given_bin_and_lib(
        &self,
        bin_name: &str,
        command: &str,
    ) -> MetadataResult<OutputWrapper> {
        Command::new("cargo")
            .arg(command)
            .arg("--bin")
            .arg(bin_name)
            .arg("--lib")
            .arg("--manifest-path")
            .arg(self.package_manifest()?)
            .arg("--message-format=json")
            .current_dir(self.package_root_dir()?)
            .output()
            .map(OutputWrapper::from)
            .map_err(MetadataError::from)
    }

    pub fn run_cargo_check(&self, bin_name: &str) -> MetadataResult<OutputWrapper> {
        self.run_cargo_command_for_given_bin_and_lib(bin_name, "check")
    }

    pub fn run_cargo_clippy(&self, bin_name: &str) -> MetadataResult<OutputWrapper> {
        self.run_cargo_command_for_given_bin_and_lib(bin_name, "clippy")
    }

    pub fn run_cargo_fmt_on_fusion_bin(
        &self,
        fusion_path: &Utf8PathBuf,
    ) -> MetadataResult<OutputWrapper> {
        Command::new("cargo")
            .arg("fmt")
            .arg("--manifest-path")
            .arg(self.package_manifest()?)
            .arg("--")
            .arg(fusion_path.as_str())
            .current_dir(self.package_root_dir()?)
            .output()
            .map(OutputWrapper::from)
            .map_err(MetadataError::from)
    }
}

pub struct OutputWrapper(Output);

impl From<Output> for OutputWrapper {
    fn from(value: Output) -> Self {
        Self(value)
    }
}

impl Deref for OutputWrapper {
    type Target = Output;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl OutputWrapper {
    pub fn collect_cargo_check_messages(&self) -> MetadataResult<()> {
        self.collect_cargo_messages("check")
    }
    pub fn collect_cargo_clippy_messages(&self) -> MetadataResult<()> {
        self.collect_cargo_messages("clippy")
    }
    pub fn display_raw_output(&self) {
        let stdout = String::from_utf8_lossy(&self.0.stdout);
        if !stdout.is_empty() {
            println!("stdout: {}", stdout);
        }
        let stderr = String::from_utf8_lossy(&self.0.stderr);
        if !stderr.is_empty() {
            println!("stderr: {}", stderr);
        }
    }
    fn collect_cargo_messages(&self, command: &str) -> MetadataResult<()> {
        // collect any remaining 'cargo <command>' messages
        let mut check_messages = String::new();

        for message in Message::parse_stream(&self.0.stdout[..]) {
            if let Message::CompilerMessage(msg) = message.context(add_context!(format!(
                "Unexpected error of parsing 'cargo {command}' messages stream."
            )))? {
                if let Some(rendered_msg) = msg.message.rendered {
                    writeln!(&mut check_messages, "{}", rendered_msg).context(add_context!(
                        format!(
                        "Unexpected error while formatting rendered 'cargo {command}' messages."
                    )
                    ))?;
                }
            }
        }
        if !check_messages.is_empty() {
            writeln!(
                &mut check_messages,
                "{}",
                String::from_utf8(self.0.stderr.to_owned()).context(add_context!(
                    "Unexpected error while converting stderr to string."
                ))?
            )
            .context(add_context!(format!(
                "Unexpected error while combining rendered 'cargo {command}' messages with stderr."
            )))?;
            return Err(MetadataError::RemainingCargoMessages(
                check_messages,
                command.to_owned(),
            ));
        }
        Ok(())
    }
}

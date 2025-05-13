use super::BinaryCommand;
use crate::{binary::ExitStatus, file_system_stdlib, manifest::ManifestField, site::Site};
use clap::{arg, value_parser, ArgMatches};
use std::path::Path;

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "manifest"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("prints a manifest value").arg(
            arg!([field] "a field to print")
                .required(true)
                .value_parser(value_parser!(ManifestField)),
        )
    }
    fn handler(
        &self,
        build_dir: &Path,
        args: &ArgMatches,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let field = args.get_one::<ManifestField>("field").unwrap();
        let fs = file_system_stdlib::NativeFileSystem::new(build_dir);
        let site = Site::load(&fs, Some(""))?;
        print!("{}", site.manifest.field_as_string(field));
        Ok(ExitStatus::Ok)
    }
}

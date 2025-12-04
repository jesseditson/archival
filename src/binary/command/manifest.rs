use super::BinaryCommand;
use crate::{
    binary::{
        command::{add_args, command_root, CommandConfig},
        ExitStatus,
    },
    file_system_stdlib,
    manifest::ManifestField,
    site::Site,
};
use clap::{arg, value_parser, ArgMatches};
use std::sync::{atomic::AtomicBool, Arc};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "manifest"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        add_args(
            cmd.about("prints a manifest value").arg(
                arg!([field] "a field to print")
                    .required(true)
                    .value_parser(value_parser!(ManifestField)),
            ),
            CommandConfig::no_build(),
        )
    }
    fn handler(
        &self,
        args: &ArgMatches,
        _quit: Arc<AtomicBool>,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let root_dir = command_root(args);
        let field = args.get_one::<ManifestField>("field").unwrap();
        let fs = file_system_stdlib::NativeFileSystem::new(&root_dir);
        let site = Site::load(&fs, Some(""))?;
        print!("{}", site.manifest.field_as_string(field));
        Ok(ExitStatus::Ok)
    }
}

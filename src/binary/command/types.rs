use super::BinaryCommand;
use crate::{
    binary::{
        command::{add_args, command_root, CommandConfig},
        ExitStatus,
    },
    file_system_stdlib,
    site::Site,
    typescript_defs::generate_typescript_defs,
    FileSystemAPI,
};
use anyhow::Result;
use clap::{arg, value_parser, ArgMatches};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "types"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        add_args(
            cmd.about("generates TypeScript definitions for this site's objects")
                .long_about(
                    "Generates TypeScript definitions for this site's objects and prints \
                     them to stdout.\n\n\
                     The output is a self-contained module exporting an `ArchivalObjects` \
                     interface, describing objects as a consumer of the built site sees \
                     them - the same shape a liquid template gets.\n\n\
                     NOTE: `secret` fields are included. They are hidden from templates, \
                     but anything consuming these types reads the real value.",
                )
                .arg(
                    arg!(-o --out <path> "write to this file instead of stdout.")
                        .required(false)
                        .value_parser(value_parser!(PathBuf)),
                ),
            CommandConfig::no_build(),
        )
    }
    fn handler(&self, args: &ArgMatches, _quit: Arc<AtomicBool>) -> Result<ExitStatus> {
        let root_dir = command_root(args);
        let mut fs = file_system_stdlib::NativeFileSystem::new(&root_dir);
        let site = Site::load(&fs, Some(""))?;
        let defs = generate_typescript_defs(&site.object_definitions, &site.root_objects(&fs));
        match args.get_one::<PathBuf>("out") {
            Some(out) => fs.write_str(out, defs)?,
            None => print!("{}", defs),
        }
        Ok(ExitStatus::Ok)
    }
}

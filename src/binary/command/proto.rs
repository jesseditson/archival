use super::BinaryCommand;
use crate::{
    archival_proto,
    binary::{
        command::{add_args, command_root, CommandConfig},
        ExitStatus,
    },
    file_system_stdlib::{self, NativeFileSystem},
    Archival,
};
use anyhow::Result;
use clap::{arg, value_parser, ArgMatches};
use prost::Message;
use std::{
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "proto"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("dumps test protos").subcommand(
            clap::Command::new("dump")
                .about("dump proto data")
                .subcommand(add_args(
                    clap::Command::new("object")
                        .about("dump an object proto")
                        .arg(
                            arg!(-o --out <file> "file to dump to")
                                .required(false)
                                .value_parser(value_parser!(PathBuf)),
                        )
                        .arg(arg!([type] "object type").required(true))
                        .arg(arg!([name] "object name").required(false)),
                    CommandConfig::no_build(),
                )),
        )
    }
    fn handler(
        &self,
        args: &ArgMatches,
        _quit: Arc<AtomicBool>,
    ) -> Result<crate::binary::ExitStatus> {
        if let Some((name, sub_m)) = args.subcommand() {
            if name == "dump" {
                if let Some((sub_name, m)) = sub_m.subcommand() {
                    if sub_name == "object" {
                        let archival = self.get_archival(m)?;
                        let name = m.get_one::<String>("type").map(|s| s.as_str()).unwrap();
                        let filename = m.get_one::<String>("name").map(|s| s.as_str());
                        let object = archival.get_object(name, filename).unwrap();
                        let obj_proto = archival_proto::Object::from(object);
                        let buffer = obj_proto.encode_to_vec();
                        if let Some(out) = m.get_one::<PathBuf>("out") {
                            println!("Writing to {}", out.display());
                            fs::write(out, buffer)?;
                        } else {
                            println!(
                                "{}",
                                String::from_utf8(buffer).expect("failed encoding to string")
                            );
                        }
                        return Ok(ExitStatus::Ok);
                    }
                }
            }
        }

        Ok(ExitStatus::Ok)
    }
}

impl Command {
    fn get_archival(&self, args: &ArgMatches) -> Result<Archival<NativeFileSystem>> {
        let root_dir = command_root(args);
        let fs = file_system_stdlib::NativeFileSystem::new(&root_dir);
        Archival::new_with_upload_prefix(fs, "")
    }
}

use super::BinaryCommand;
use crate::{
    binary::{
        command::{add_args, CommandConfig},
        ExitStatus,
    },
    check_compatibility,
};
use anyhow::Result;
use clap::{arg, value_parser, ArgMatches};
use std::sync::{atomic::AtomicBool, Arc};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "compat"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        add_args(
            cmd.about(
                "checks the compatibility of this version of archival against a version string",
            )
            .arg(
                arg!([version] "a version string (e.g. 0.4.1-alpha)")
                    .required(true)
                    .value_parser(value_parser!(String)),
            ),
            CommandConfig::default(),
        )
    }
    fn handler(
        &self,
        args: &ArgMatches,
        _quit: Arc<AtomicBool>,
    ) -> Result<crate::binary::ExitStatus> {
        let (compat, message) = check_compatibility(args.get_one::<String>("version").unwrap());
        println!("{}", message);
        match compat {
            true => Ok(ExitStatus::Ok),
            false => Ok(ExitStatus::Error),
        }
    }
}

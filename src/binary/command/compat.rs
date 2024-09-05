use super::BinaryCommand;
use crate::{binary::ExitStatus, check_compatibility};
use clap::{arg, value_parser, ArgMatches};
use std::path::Path;

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "compat"
    }
    fn no_path(&self) -> bool {
        true
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("checks the compatibility of this version of archival against a version string")
            .arg(
                arg!([version] "a version string (e.g. 0.4.1-alpha)")
                    .required(true)
                    .value_parser(value_parser!(String)),
            )
    }
    fn handler(
        &self,
        _build_dir: &Path,
        args: &ArgMatches,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let (compat, message) = check_compatibility(args.get_one::<String>("version").unwrap());
        println!("{}", message);
        match compat {
            true => Ok(ExitStatus::Ok),
            false => Ok(ExitStatus::Error),
        }
    }
}

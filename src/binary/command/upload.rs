use super::BinaryCommand;
use crate::binary::ExitStatus;
use clap::{arg, value_parser, ArgMatches};
use std::path::{Path, PathBuf};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "upload"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("uploads a local file to archival").arg(
            arg!([file] "The file to upload")
                .required(true)
                .value_parser(value_parser!(PathBuf)),
            // TODO: allow overriding type
            // TODO: allow choosing object(s) and fields to set
        )
    }
    fn handler(
        &self,
        _build_dir: &Path,
        args: &ArgMatches,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let _field = args.get_one::<PathBuf>("file").unwrap();
        // TODO: make sure we're authenticated
        // TODO: parse file type
        // TODO: upload the file to the archival CDN
        // TODO: generate a File object and either print the toml to stdout or
        // write it to the specified file
        Ok(ExitStatus::Ok)
    }
}

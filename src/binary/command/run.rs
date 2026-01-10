use super::BinaryCommand;
use crate::binary::command::{add_args, command_root, CommandConfig};
use crate::binary::dev_server::{self, DevServerMode, UploadsConfig};
use clap::{arg, value_parser, ArgMatches};
use std::sync::{atomic::AtomicBool, Arc};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "run"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        add_args(
            cmd.about("auto-rebuild an archival site")
                .arg(
                    arg!(-p --port <port> "static server port")
                        .required(false)
                        .value_parser(value_parser!(u16)),
                )
                .arg(arg!(-n --noserve "disables the static server").required(false)),
            CommandConfig::archival_site(),
        )
    }
    fn handler(
        &self,
        args: &ArgMatches,
        quit: Arc<AtomicBool>,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let root_dir = command_root(args);
        let upload_prefix = args.get_one::<String>("upload-prefix").map(|s| s.as_str());
        let options = if !args.get_one::<bool>("noserve").unwrap() {
            DevServerMode::Serve(args.get_one::<u16>("port").copied())
        } else {
            DevServerMode::NoServe
        };
        dev_server::watch(
            root_dir,
            upload_prefix.map(UploadsConfig::prefix).unwrap_or_default(),
            options,
            None,
            None,
            quit,
        )
    }
}

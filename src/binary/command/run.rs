use super::BinaryCommand;
#[cfg(feature = "carriers")]
use crate::binary::carriers::CarrierOptions;
use crate::binary::command::{add_args, command_root, CommandConfig};
use crate::binary::dev_server::{self, DevServerMode, DevServerOptions, UploadsConfig};
use anyhow::Result;
use clap::{arg, value_parser, ArgMatches};
use std::sync::{atomic::AtomicBool, Arc};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "run"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        let cmd = cmd
            .about("auto-rebuild an archival site")
            .arg(
                arg!(-p --port <port> "static server port")
                    .required(false)
                    .value_parser(value_parser!(u16)),
            )
            .arg(arg!(-n --noserve "disables the static server").required(false));
        #[cfg(feature = "carriers")]
        let cmd = cmd
            .arg(
                arg!(--"no-carriers" "don't run the carriers in this site's carriers/ directory")
                    .required(false),
            )
            .arg(
                arg!(--"carriers-port" <port> "pin the carrier sidecar's port")
                    .required(false)
                    .value_parser(value_parser!(u16)),
            )
            .arg(
                arg!(--"carriers-inspect" "run the carrier sidecar with node --inspect")
                    .required(false),
            )
            .arg(
                arg!(--"site-url" <url> "override the SITE_URL carriers see")
                    .required(false)
                    .value_parser(value_parser!(String)),
            );
        add_args(cmd, CommandConfig::archival_site())
    }
    fn handler(
        &self,
        args: &ArgMatches,
        quit: Arc<AtomicBool>,
    ) -> Result<crate::binary::ExitStatus> {
        let root_dir = command_root(args);
        let upload_prefix = args.get_one::<String>("upload-prefix").map(|s| s.as_str());
        let options = if !args.get_one::<bool>("noserve").unwrap() {
            DevServerMode::Serve(args.get_one::<u16>("port").copied())
        } else {
            DevServerMode::NoServe
        };
        dev_server::watch_with(DevServerOptions {
            root_dir,
            uploads_config: upload_prefix.map(UploadsConfig::prefix).unwrap_or_default(),
            mode: options,
            change_sender: None,
            watch_paths: None,
            quit,
            #[cfg(feature = "carriers")]
            carriers: CarrierOptions {
                disabled: *args.get_one::<bool>("no-carriers").unwrap(),
                node_args: if *args.get_one::<bool>("carriers-inspect").unwrap() {
                    vec!["--inspect".to_string()]
                } else {
                    vec![]
                },
                port: args.get_one::<u16>("carriers-port").copied(),
            },
            #[cfg(feature = "carriers")]
            site_url: args.get_one::<String>("site-url").cloned(),
        })
    }
}

use super::BinaryCommand;
use crate::{
    binary::{
        carriers::{discovery, objects::build_payload},
        command::{add_args, command_root, CommandConfig},
        ExitStatus,
    },
    file_system_stdlib,
    site::Site,
};
use anyhow::Result;
use clap::{arg, ArgMatches};
use std::sync::{atomic::AtomicBool, Arc};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "carriers"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("inspects this site's carriers")
            .subcommand_required(true)
            .subcommand(add_args(
                clap::Command::new("list")
                    .about("lists this site's carriers and their entry files"),
                CommandConfig::no_build(),
            ))
            .subcommand(add_args(
                clap::Command::new("objects")
                    .about("prints the objects carriers receive, as JSON")
                    .long_about(
                        "Prints the third argument every carrier receives: this site's \
                         objects, in the same shape `archival types` describes.\n\n\
                         NOTE: `secret` fields are included. They are hidden from \
                         templates, but carriers run on the server and read the real \
                         value.",
                    )
                    .arg(
                        arg!(--"site-url" <url> "the SITE_URL carriers see")
                            .required(false)
                            .default_value(""),
                    ),
                CommandConfig::archival_site(),
            ))
    }
    fn handler(&self, args: &ArgMatches, _quit: Arc<AtomicBool>) -> Result<ExitStatus> {
        match args.subcommand() {
            Some(("list", args)) => {
                let root_dir = command_root(args);
                let carriers = discovery::discover(&root_dir)?;
                if carriers.is_empty() {
                    println!("no carriers found in {}", root_dir.display());
                }
                for carrier in carriers {
                    match carrier.entry() {
                        Some(entry) => println!(
                            "{}\t{}",
                            carrier.name,
                            entry.strip_prefix(&root_dir).unwrap_or(&entry).display()
                        ),
                        None => println!(
                            "{}\tno entry file - expected {}",
                            carrier.name,
                            carrier.expected_entry()
                        ),
                    }
                }
            }
            Some(("objects", args)) => {
                let root_dir = command_root(args);
                let upload_prefix = args.get_one::<String>("upload-prefix").map(|s| s.as_str());
                let fs = file_system_stdlib::NativeFileSystem::new(&root_dir);
                let site = Site::load(&fs, upload_prefix)?;
                let site_url = args.get_one::<String>("site-url").unwrap();
                let payload = build_payload(&site, &fs, site_url)?;
                println!("{}", serde_json::to_string_pretty(&payload)?);
            }
            _ => unreachable!("subcommand_required"),
        }
        Ok(ExitStatus::Ok)
    }
}

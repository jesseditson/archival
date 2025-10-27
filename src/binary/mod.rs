pub mod command;
pub mod config;
use self::command::{ExitStatus, COMMANDS};
use clap::{arg, command, value_parser, Command};
pub use config::ArchivalConfig;
use std::{
    env,
    error::Error,
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, LazyLock},
};

static CWD: LazyLock<PathBuf> = std::sync::LazyLock::new(|| env::current_dir().unwrap());

pub fn binary(
    args: impl Iterator<Item = String>,
    quit: Option<Arc<AtomicBool>>,
) -> Result<ExitStatus, Box<dyn Error>> {
    let mut cmd = command!().arg_required_else_help(true);
    for command in COMMANDS {
        let mut subcommand = command.cli(Command::new(command.name()));
        if command.uses_uploads() {
            subcommand = subcommand.arg(
                // NOTE: weird long form quoting due to https://github.com/clap-rs/clap/issues/3586
                arg!(-u --"upload-prefix" <upload_prefix> "override the upload prefix. If no manifest.toml is present, this is required.")
                    .value_parser(value_parser!(String)),
            )
        }
        if !command.no_path() {
            subcommand = subcommand.arg(
                arg!([site_path] "an optional path to the archival site. Otherwise will be auto-detected from cwd.")
                    .required(false)
                    .default_value(CWD.to_str())
                    .value_parser(value_parser!(PathBuf)),
            );
        }
        cmd = cmd.subcommand(subcommand);
    }
    let matches = cmd.get_matches_from(args);
    let quit = if let Some(quit) = quit {
        quit
    } else {
        Arc::new(AtomicBool::default())
    };
    COMMANDS
        .iter()
        .find_map(|c| {
            if let Some(args) = matches.subcommand_matches(c.name()) {
                let build_dir = if !c.no_path() {
                    let path = args.get_one::<PathBuf>("site_path").unwrap(); // defaulted so unwrap is ok
                    if *path != *CWD {
                        // If not default, the path is relative
                        fs::canonicalize(CWD.join(path)).unwrap()
                    } else {
                        CWD.to_path_buf()
                    }
                } else {
                    CWD.to_path_buf()
                };
                Some(c.handler(&build_dir, args, quit.clone()))
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("No command provided."))
}

pub mod command;
mod config;
use self::command::{ExitStatus, COMMANDS};
use clap::{arg, command, value_parser, Command};
pub use config::ArchivalConfig;
use std::{env, error::Error, fs, path::PathBuf};

pub fn binary(args: impl Iterator<Item = String>) -> Result<ExitStatus, Box<dyn Error>> {
    let build_dir = env::current_dir()?;
    let mut cmd = command!().arg_required_else_help(true);
    for command in COMMANDS {
        let mut subcommand = command.cli(Command::new(command.name()));
        if command.has_path() {
            subcommand = subcommand.arg(
                arg!([site_path] "an optional path to the archival site. Otherwise will be auto-detected from cwd.")
                    .required(false)
                    .value_parser(value_parser!(PathBuf)),
            );
        }
        cmd = cmd.subcommand(subcommand);
    }
    let matches = cmd.get_matches_from(args);
    COMMANDS
        .iter()
        .find_map(|c| {
            if let Some(args) = matches.subcommand_matches(c.name()) {
                let build_dir = if c.has_path() {
                    if let Some(path) = args.get_one::<PathBuf>("site_path") {
                        fs::canonicalize(build_dir.join(path)).unwrap()
                    } else {
                        build_dir.to_path_buf()
                    }
                } else {
                    build_dir.to_path_buf()
                };
                Some(c.handler(&build_dir, args))
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("No command provided."))
}

pub mod command;
pub mod config;

use self::command::{ExitStatus, COMMANDS};
use clap::{command, Command};
pub use config::ArchivalConfig;
use std::{
    env,
    error::Error,
    sync::{atomic::AtomicBool, Arc},
};

pub fn binary(
    args: impl Iterator<Item = String>,
    quit: Option<Arc<AtomicBool>>,
) -> Result<ExitStatus, Box<dyn Error>> {
    let mut cmd: Command = command!().arg_required_else_help(true);
    for command in COMMANDS {
        cmd = cmd.subcommand(command.cli(Command::new(command.name())));
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
            matches
                .subcommand_matches(c.name())
                .map(|args| c.handler(args, quit.clone()))
        })
        .unwrap_or_else(|| panic!("No command provided."))
}

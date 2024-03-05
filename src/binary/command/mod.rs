use clap::{ArgMatches, Command};
use std::{error::Error, path::Path};
mod build;
mod compat;
mod login;
mod manifest;
mod prebuild;
mod run;

pub enum ExitStatus {
    Error,
    Ok,
}
impl ExitStatus {
    pub fn code(&self) -> i32 {
        match self {
            ExitStatus::Error => 1,
            ExitStatus::Ok => 0,
        }
    }
}

pub trait BinaryCommand {
    fn name(&self) -> &str;
    fn has_path(&self) -> bool {
        true
    }
    fn cli(&self, cmd: Command) -> Command;
    fn handler(&self, build_dir: &Path, args: &ArgMatches) -> Result<ExitStatus, Box<dyn Error>>;
}

pub const COMMANDS: [&dyn BinaryCommand; 6] = [
    &build::Command {},
    &run::Command {},
    &manifest::Command {},
    &prebuild::Command {},
    &login::Command {},
    &compat::Command {},
];

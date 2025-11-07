use clap::{ArgMatches, Command};
use std::{
    error::Error,
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};
mod build;
mod compat;
mod format;
mod import;
mod login;
mod manifest;
mod objects;
mod prebuild;
mod run;
#[cfg(feature = "json-schema")]
mod schemas;
mod upload;

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
    fn no_path(&self) -> bool {
        false
    }
    fn uses_uploads(&self) -> bool {
        false
    }
    fn cli(&self, cmd: Command) -> Command;
    fn handler(
        &self,
        build_dir: &Path,
        args: &ArgMatches,
        quit: Arc<AtomicBool>,
    ) -> Result<ExitStatus, Box<dyn Error>>;
}

pub const COMMANDS: [&dyn BinaryCommand; 11] = [
    &build::Command {},
    &run::Command {},
    &format::Command {},
    &manifest::Command {},
    &prebuild::Command {},
    &login::Command {},
    &compat::Command {},
    &upload::Command {},
    &import::Command {},
    &objects::Command {},
    &schemas::Command {},
];

use super::BinaryCommand;
use crate::{binary::ExitStatus, file_system_stdlib, site::Site};
use clap::ArgMatches;
use std::path::Path;

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "build"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("builds an archival site")
    }
    fn handler(
        &self,
        build_dir: &Path,
        _args: &ArgMatches,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let mut fs = file_system_stdlib::NativeFileSystem::new(build_dir);
        let site = Site::load(&fs)?;
        println!("Building site: {}", &site);
        site.build(&mut fs)?;
        Ok(ExitStatus::Ok)
    }
}

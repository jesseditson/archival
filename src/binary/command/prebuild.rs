use super::BinaryCommand;
use crate::{binary::ExitStatus, file_system_stdlib, site::Site};
use clap::ArgMatches;
use std::path::Path;

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "prebuild"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("runs external build commands, if configured.")
    }
    fn handler(
        &self,
        build_dir: &Path,
        _args: &ArgMatches,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let fs = file_system_stdlib::NativeFileSystem::new(build_dir);
        let site = Site::load(&fs)?;
        for s in site.manifest.prebuild {
            let cmd_parts: Vec<&str> = s.split_whitespace().collect();
            if !cmd_parts.is_empty() {
                println!("runnning {} {}", cmd_parts[0], cmd_parts[1..].join(" "));
                let status = std::process::Command::new(cmd_parts[0])
                    .args(&cmd_parts[1..])
                    .current_dir(build_dir)
                    .spawn()
                    .unwrap_or_else(|_| panic!("spawn failed: {}", s))
                    .wait();
                match status {
                    Ok(status) => {
                        if !status.success() {
                            return Ok(ExitStatus::Error);
                        }
                    }
                    Err(e) => {
                        println!("error: {}", e);
                        return Ok(ExitStatus::Error);
                    }
                }
            }
        }
        Ok(ExitStatus::Ok)
    }
}

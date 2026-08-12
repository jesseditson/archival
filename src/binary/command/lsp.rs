use super::BinaryCommand;
use crate::binary::ExitStatus;
use anyhow::Result;
use clap::ArgMatches;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "lsp"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("runs a language server for archival templates over stdio")
            .arg(
                clap::Arg::new("stdio")
                    .long("stdio")
                    .action(clap::ArgAction::SetTrue)
                    .help("Accepted for compatibility; stdio is the only transport"),
            )
    }
    fn handler(&self, _args: &ArgMatches, quit: Arc<AtomicBool>) -> Result<ExitStatus> {
        crate::lsp::run_stdio(quit)
    }
}

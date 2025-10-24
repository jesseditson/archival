use super::BinaryCommand;
use crate::{binary::ExitStatus, file_system_stdlib, site::Site, FieldConfig};
use clap::{arg, value_parser, ArgMatches};
use std::path::{Path, PathBuf};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "build"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("builds an archival site").arg(
            // NOTE: weird long form quoting due to https://github.com/clap-rs/clap/issues/3586
            arg!(-b --"build-dir" <build_dir> "Override the directory to build to (defaults to the manifest's build_dir)")
                .value_parser(value_parser!(PathBuf)),
        )
    }
    fn uses_uploads(&self) -> bool {
        true
    }
    fn handler(
        &self,
        root_dir: &Path,
        args: &ArgMatches,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let mut fs = file_system_stdlib::NativeFileSystem::new(root_dir);
        let mut site = Site::load(
            &fs,
            args.get_one::<String>("upload-prefix").map(|s| s.as_str()),
        )?;
        FieldConfig::set_global(site.get_field_config(None)?);
        println!("Building site: {}", &site);
        if let Some(build_dir_arg) = args.get_one::<PathBuf>("build-dir") {
            let cwd = std::env::current_dir().unwrap();
            site.manifest.build_dir = cwd.join(build_dir_arg);
        }
        site.sync_static_files(&mut fs)?;
        site.build(&mut fs)?;
        Ok(ExitStatus::Ok)
    }
}

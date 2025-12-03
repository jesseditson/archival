use super::BinaryCommand;
use crate::{
    binary::{command::command_root, ExitStatus},
    file_system_stdlib,
    json_schema::{generate_json_schema, generate_root_json_schema, ObjectSchemaOptions},
    site::Site,
};
use clap::{arg, value_parser, ArgMatches};
use std::sync::{atomic::AtomicBool, Arc};

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum SchemaError {
    #[error("object {0} doesn't exist")]
    NoObject(String),
}

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "schemas"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("dumps schemas").arg(
            arg!(-o --object <name> "an object name. If not provided, will dump all objects.")
                .required(false)
                .value_parser(value_parser!(String)),
        )
        .arg(
            arg!(-i --inline "If provided, this will print the schema to stdout instead of generating schema files.").required(false),
        )
        .arg(
            arg!(-p --pretty "If provided, will prettify json before printing or dumping.").required(false),
        )
    }
    fn handler(
        &self,
        args: &ArgMatches,
        _quit: Arc<AtomicBool>,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let root_dir = command_root(args);
        let mut fs = file_system_stdlib::NativeFileSystem::new(&root_dir);
        let object = args.get_one::<String>("object");
        let inline = *args.get_one::<bool>("inline").unwrap();
        let pretty = *args.get_one::<bool>("pretty").unwrap();
        let site = Site::load(&fs, Some(""))?;
        if inline {
            let schema = if let Some(object) = object {
                let def = site
                    .object_definitions
                    .get(object)
                    .ok_or_else(|| SchemaError::NoObject(object.clone()))?;
                generate_json_schema(
                    &format!("{}/{}.schema.json", site.schema_prefix(), object),
                    def,
                    ObjectSchemaOptions::default(),
                )
            } else {
                generate_root_json_schema(
                    &format!("{}/root.schema.json", site.schema_prefix()),
                    site.manifest.site_name.as_deref(),
                    &format!(
                        "Object definitions{}",
                        if let Some(site_name) = &site.manifest.site_name {
                            format!(" for {}", site_name)
                        } else {
                            "".to_string()
                        }
                    ),
                    &site.object_definitions,
                    &site.root_objects(&fs),
                    ObjectSchemaOptions::default(),
                )
            };
            println!(
                "{}",
                if pretty {
                    serde_json::to_string_pretty(&schema).unwrap()
                } else {
                    serde_json::to_string(&schema).unwrap()
                }
            );
        } else if let Some(object) = object {
            site.dump_schema(object, &mut fs)?;
        } else {
            site.dump_schemas(&mut fs)?;
        }
        Ok(ExitStatus::Ok)
    }
}

use super::BinaryCommand;
use crate::{
    binary::{
        command::{add_args, command_root, CommandConfig},
        ExitStatus,
    },
    file_system_stdlib,
    object::ObjectEntry,
    page::debug_context,
    site::Site,
};
use clap::ArgMatches;
use liquid_core::Value;
use ordermap::OrderMap;
use std::sync::{atomic::AtomicBool, Arc};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "objects"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        add_args(
            cmd.about("lists the objects in this site"),
            CommandConfig::no_build(),
        )
    }
    fn handler(
        &self,
        args: &ArgMatches,
        _quit: Arc<AtomicBool>,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let root_dir = command_root(args);
        let fs = file_system_stdlib::NativeFileSystem::new(&root_dir);
        let site = Site::load(&fs, Some(""))?;
        let mut objects: OrderMap<String, liquid::model::Value> = OrderMap::new();
        let definitions = &site.object_definitions;
        for (name, obj_entry) in site.get_objects(&fs)? {
            let definition = definitions
                .get(&name)
                .unwrap_or_else(|| panic!("missing object definition {}", name));
            let values = match obj_entry {
                ObjectEntry::List(l) => Value::array(
                    l.iter()
                        .map(|o| o.liquid_object(definition, &site.field_config)),
                ),
                ObjectEntry::Object(o) => o.liquid_object(definition, &site.field_config),
            };
            objects.insert(name.to_string(), values);
        }
        println!(
            "{}",
            debug_context(&liquid::object!({"objects": objects}), 0)
        );
        // let page = Page::new(
        //     "objects-template",
        //     "",
        //     TemplateType::Default,
        //     &"",
        // );
        // let render_o = page.render(&liquid_parser, &all_objects);
        Ok(ExitStatus::Ok)
        // match compat {
        //     true => Ok(ExitStatus::Ok),
        //     false => Ok(ExitStatus::Error),
        // }
    }
}

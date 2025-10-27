use super::BinaryCommand;
use crate::{
    binary::ExitStatus, file_system_stdlib, object::ObjectEntry, page::debug_context, site::Site,
    FieldConfig,
};
use clap::ArgMatches;
use liquid_core::Value;
use std::{
    collections::HashMap,
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "objects"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("lists the objects in this site")
    }
    fn handler(
        &self,
        build_dir: &Path,
        _args: &ArgMatches,
        _quit: Arc<AtomicBool>,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        let fs = file_system_stdlib::NativeFileSystem::new(build_dir);
        let site = Site::load(&fs, Some(""))?;
        FieldConfig::set_global(site.get_field_config(None)?);
        let mut objects: HashMap<String, liquid::model::Value> = HashMap::new();
        let definitions = &site.object_definitions;
        for (name, obj_entry) in site.get_objects(&fs)? {
            let definition = definitions
                .get(&name)
                .unwrap_or_else(|| panic!("missing object definition {}", name));
            let values = match obj_entry {
                ObjectEntry::List(l) => Value::array(l.iter().map(|o| o.liquid_object(definition))),
                ObjectEntry::Object(o) => o.liquid_object(definition),
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

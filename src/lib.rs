use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

mod field_value;
mod liquid_parser;
mod manifest;
mod object;
mod object_definition;
mod page;
mod read_toml;
mod reserved_fields;

use constants::MANIFEST_FILE_NAME;
use manifest::Manifest;
use object::Object;
use object_definition::{ObjectDefinition, ObjectDefinitions};
use page::Page;
use read_toml::read_toml;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

mod constants;

#[derive(Deserialize, Serialize)]
pub struct Site {
    pub root: PathBuf,
    pub objects: ObjectDefinitions,
    pub manifest: Manifest,
}

pub fn load_site(root: &Path) -> Result<Site, Box<dyn Error>> {
    // Load our manifest (should it exist)
    let manifest = match Manifest::from_file(&root.join(MANIFEST_FILE_NAME)) {
        Ok(m) => m,
        Err(_) => Manifest::default(root),
    };
    // Load our object definitions
    let objects_table = read_toml(&manifest.object_definition_file)?;
    let objects = ObjectDefinition::from_table(&objects_table)?;
    Ok(Site {
        root: root.to_path_buf(),
        manifest,
        objects,
    })
}

pub fn build_site(site: &Site) -> Result<(), Box<dyn Error>> {
    let mut all_objects: HashMap<String, Vec<Object>> = HashMap::new();
    let objects_dir = site.root.join(&site.manifest.objects_dir);
    for (object_name, object_def) in site.objects.iter() {
        let mut objects: Vec<Object> = Vec::new();
        let object_files_dir = objects_dir.join(object_name);
        if objects_dir.is_dir() {
            for file in fs::read_dir(object_files_dir)? {
                let file = file?;
                if file.path().ends_with(".toml") {
                    let obj_table = read_toml(&file.path())?;
                    objects.push(Object::from_table(object_def, object_name, &obj_table)?)
                }
            }
        }
        // Sort objects by order key
        objects.sort_by(|a, b| a.order.partial_cmp(&b.order).unwrap());
        all_objects.insert(object_name.clone(), objects);
    }
    let liquid_parser = liquid_parser::get();
    let pages_dir = site.root.join(&site.manifest.pages_dir);
    let build_dir = site.root.join(&site.manifest.build_dir);
    // Render template pages
    for (name, object_def) in site.objects.iter() {
        if let Some(template) = &object_def.template {
            if let Some(t_objects) = all_objects.get(name) {
                for object in t_objects {
                    let template_path = pages_dir.join(format!("{}.liquid", template));
                    let template_str = fs::read_to_string(&template_path)?;
                    let page = Page::new_with_template(
                        object.name.clone(),
                        object_def,
                        object,
                        template_str,
                    );
                    let rendered = page.render(&liquid_parser, &all_objects)?;
                    let render_name = format!("{}.html", object.name);
                    let build_path = build_dir.join(render_name);
                    fs::write(build_path, rendered)?;
                }
            }
        }
    }
    // Render regular pages
    for file in WalkDir::new(pages_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_name = file.file_name().to_string_lossy();
        if file_name.ends_with(".liquid") {
            let page_name = file_name.replace(".liquid", "");
            let template_str = fs::read_to_string(&file.path())?;
            let page = Page::new(page_name, template_str);
            let rendered = page.render(&liquid_parser, &all_objects)?;
            let render_name = file_name.replace(".liquid", ".html");
            fs::write(build_dir.join(render_name), rendered)?;
        }
    }
    Ok(())
}

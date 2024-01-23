use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
};

use crate::{
    constants::MANIFEST_FILE_NAME,
    file_system_mutex::FileSystemMutex,
    liquid_parser,
    manifest::Manifest,
    object::Object,
    object_definition::{ObjectDefinition, ObjectDefinitions},
    page::Page,
    read_toml::read_toml,
    tags::layout,
    ArchivalError, FileSystemAPI,
};

#[derive(Deserialize, Serialize)]
pub struct Site {
    pub root: PathBuf,
    pub objects: ObjectDefinitions,
    pub manifest: Manifest,
}

impl std::fmt::Display for Site {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"
        === Root:
            {}
        === Objects:
            {}
        === Manifest: {}
        "#,
            self.root.display(),
            self.objects
                .keys()
                .map(|o| o.as_str().to_string())
                .collect::<Vec<String>>()
                .join("\n"),
            self.manifest
        )
    }
}

pub fn load(root: &Path, fs: &impl FileSystemAPI) -> Result<Site, Box<dyn Error>> {
    // Load our manifest (should it exist)
    let manifest = match Manifest::from_file(&root.join(MANIFEST_FILE_NAME), fs) {
        Ok(m) => m,
        Err(_) => Manifest::default(root),
    };
    println!("m: {}", manifest);
    println!("fs: {:?}", root);
    let odf = root.join(&manifest.object_definition_file);
    if !fs.exists(&odf)? {
        return Err(ArchivalError::new(&format!(
            "Object definition file {} does not exist",
            odf.to_string_lossy()
        ))
        .into());
    }

    // Load our object definitions
    let objects_table = read_toml(&odf, fs)?;
    let objects = ObjectDefinition::from_table(&objects_table)?;
    Ok(Site {
        root: root.to_path_buf(),
        manifest,
        objects,
    })
}

pub fn build<T: FileSystemAPI>(site: &Site, fs: FileSystemMutex<T>) -> Result<(), Box<dyn Error>> {
    let mut all_objects: HashMap<String, Vec<Object>> = HashMap::new();
    let objects_dir = site.root.join(&site.manifest.objects_dir);
    let layout_dir = site.root.join(&site.manifest.layout_dir);
    let pages_dir = site.root.join(&site.manifest.pages_dir);
    let build_dir = site.root.join(&site.manifest.build_dir);
    let static_dir = site.root.join(&site.manifest.static_dir);

    // Validate paths
    fs.with_fs(|fs| {
        if !fs.exists(&objects_dir)? {
            return Err(ArchivalError::new(&format!(
                "Objects dir {} does not exist",
                objects_dir.to_string_lossy()
            ))
            .into());
        }
        if !fs.exists(&pages_dir)? {
            return Err(ArchivalError::new(&format!(
                "Pages dir {} does not exist",
                pages_dir.to_string_lossy()
            ))
            .into());
        }
        if !fs.exists(&build_dir)? {
            fs.create_dir_all(&build_dir)?;
        }

        // Copy static files
        if fs.exists(&static_dir)? {
            fs.copy_contents(&static_dir, &build_dir)?;
        }
        Ok(())
    })?;

    for (object_name, object_def) in site.objects.iter() {
        let mut objects: Vec<Object> = Vec::new();
        let object_files_dir = objects_dir.join(object_name);
        if objects_dir.is_dir() {
            for file in fs.with_fs(|f| f.read_dir(&object_files_dir))? {
                if file.ends_with(".toml") {
                    let obj_table = fs.with_fs(|fs| read_toml(&file, fs))?;
                    objects.push(Object::from_table(object_def, object_name, &obj_table)?)
                }
            }
        }
        // Sort objects by order key
        objects.sort_by(|a, b| a.order.partial_cmp(&b.order).unwrap());
        all_objects.insert(object_name.clone(), objects);
    }

    let liquid_parser = liquid_parser::get(if layout_dir.exists() {
        Some(layout_dir)
    } else {
        None
    })?;

    // Render template pages
    for (name, object_def) in site.objects.iter() {
        if let Some(template) = &object_def.template {
            if let Some(t_objects) = all_objects.get(name) {
                for object in t_objects {
                    let template_path = pages_dir.join(format!("{}.liquid", template));
                    let template_str = fs.with_fs(|f| f.read_to_string(&template_path))?;
                    if let Some(template_str) = template_str {
                        let page = Page::new_with_template(
                            object.name.clone(),
                            object_def,
                            object,
                            template_str,
                        );
                        let rendered =
                            layout::post_process(page.render(&liquid_parser, &all_objects)?);
                        let render_name = format!("{}.html", object.name);
                        let build_path = build_dir.join(render_name);
                        fs.with_fs(|f| f.write_str(&build_path, rendered))?;
                    }
                }
            }
        }
    }
    // Render regular pages
    for file in fs.with_fs(|f| f.walk_dir(&pages_dir))? {
        if let Some(name) = file.file_name() {
            let file_name = name.to_string_lossy();
            if file_name.ends_with(".liquid") {
                let page_name = file_name.replace(".liquid", "");
                if let Some(template_str) = fs.with_fs(|f| f.read_to_string(file.as_path()))? {
                    let page = Page::new(page_name, template_str);
                    let rendered = layout::post_process(page.render(&liquid_parser, &all_objects)?);
                    let render_name = file_name.replace(".liquid", ".html");
                    fs.with_fs(|f| f.write_str(&build_dir.join(render_name), rendered))?;
                } else {
                    println!("template not found: {}", file.display());
                }
            }
        }
    }

    Ok(())
}

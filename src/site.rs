use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    error::Error,
    path::Path,
};
use tracing::debug;

use crate::{
    constants::MANIFEST_FILE_NAME,
    liquid_parser::{self, partial_matcher},
    manifest::Manifest,
    object::Object,
    object_definition::{ObjectDefinition, ObjectDefinitions},
    page::Page,
    read_toml::read_toml,
    tags::layout,
    ArchivalError, FileSystemAPI,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct Site {
    pub objects: ObjectDefinitions,
    pub manifest: Manifest,
}

impl std::fmt::Display for Site {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"
    === Objects:
        {}
    === Manifest: {}
        "#,
            self.objects
                .keys()
                .map(|o| o.as_str().to_string())
                .collect::<Vec<String>>()
                .join("\n        "),
            self.manifest
        )
    }
}

fn get_order(obj: &Object) -> String {
    if obj.order == -1 {
        obj.filename.to_owned()
    } else {
        format!("{:0>10}", obj.order)
    }
}

pub fn load(fs: &impl FileSystemAPI) -> Result<Site, Box<dyn Error>> {
    // Load our manifest (should it exist)
    let manifest = match Manifest::from_file(Path::new(MANIFEST_FILE_NAME), fs) {
        Ok(m) => m,
        Err(_) => Manifest::default(Path::new("")),
    };
    let odf = Path::new(&manifest.object_definition_file);
    if !fs.exists(odf)? {
        return Err(ArchivalError::new(&format!(
            "Object definition file {} does not exist",
            odf.to_string_lossy()
        ))
        .into());
    }

    // Load our object definitions
    let objects_table = read_toml(odf, fs)?;
    let objects = ObjectDefinition::from_table(&objects_table)?;
    Ok(Site { manifest, objects })
}

pub fn get_objects<T: FileSystemAPI>(
    site: &Site,
    fs: &T,
) -> Result<HashMap<String, Vec<Object>>, Box<dyn Error>> {
    get_objects_sorted(site, fs, |a, b| {
        get_order(a).partial_cmp(&get_order(b)).unwrap()
    })
}

pub fn get_objects_sorted<T: FileSystemAPI>(
    site: &Site,
    fs: &T,
    sort: impl Fn(&Object, &Object) -> Ordering,
) -> Result<HashMap<String, Vec<Object>>, Box<dyn Error>> {
    let mut all_objects: HashMap<String, Vec<Object>> = HashMap::new();
    let objects_dir = &site.manifest.objects_dir;
    for (object_name, object_def) in site.objects.iter() {
        let mut objects: Vec<Object> = Vec::new();
        let object_files_dir = objects_dir.join(object_name);
        if fs.is_dir(objects_dir)? {
            for file in fs.walk_dir(&object_files_dir, false)? {
                if let Some(ext) = file.extension() {
                    if ext == "toml" {
                        let obj_table = read_toml(&object_files_dir.join(&file), fs)?;
                        objects.push(Object::from_table(
                            object_def,
                            &file
                                .with_extension("")
                                .file_name()
                                .unwrap()
                                .to_string_lossy()
                                .to_lowercase(),
                            &obj_table,
                        )?)
                    }
                }
            }
        }
        // Sort objects by order key
        objects.sort_by(&sort);
        all_objects.insert(object_name.clone(), objects);
    }
    Ok(all_objects)
}

pub fn build<T: FileSystemAPI>(site: &Site, fs: &mut T) -> Result<(), Box<dyn Error>> {
    let objects_dir = &site.manifest.objects_dir;
    let layout_dir = &site.manifest.layout_dir;
    let pages_dir = &site.manifest.pages_dir;
    let build_dir = &site.manifest.build_dir;
    let static_dir = &site.manifest.static_dir;

    // Validate paths
    if !fs.exists(objects_dir)? {
        return Err(ArchivalError::new(&format!(
            "Objects dir {} does not exist",
            objects_dir.to_string_lossy()
        ))
        .into());
    }
    if !fs.exists(pages_dir)? {
        return Err(ArchivalError::new(&format!(
            "Pages dir {} does not exist",
            pages_dir.to_string_lossy()
        ))
        .into());
    }
    if !fs.exists(build_dir)? {
        fs.create_dir_all(build_dir)?;
    }

    // Copy static files
    debug!("copying files from {}", static_dir.display());
    if fs.exists(static_dir)? {
        fs.copy_recursive(static_dir, build_dir)?;
    } else {
        debug!("static dir {} does not exist.", static_dir.display());
    }

    let all_objects = get_objects(site, fs)?;

    // for (n, os) in &all_objects {
    //     debug!("{}", n);
    //     for o in os {
    //         debug!("{}", o.filename);
    //         for (k, v) in &o.values {
    //             debug!("{}: {:?}", k, v);
    //         }
    //     }
    // }

    let liquid_parser = liquid_parser::get(
        Some(pages_dir),
        if fs.exists(layout_dir)? {
            Some(layout_dir)
        } else {
            None
        },
        fs,
    )?;

    // Render template pages
    for (name, object_def) in site.objects.iter() {
        if let Some(template) = &object_def.template {
            if let Some(t_objects) = all_objects.get(name) {
                for object in t_objects {
                    let template_path = pages_dir.join(format!("{}.liquid", template));
                    let template_str = fs.read_to_string(&template_path)?;
                    if let Some(template_str) = template_str {
                        let page = Page::new_with_template(
                            object.filename.clone(),
                            object_def,
                            object,
                            template_str,
                        );
                        let rendered =
                            layout::post_process(page.render(&liquid_parser, &all_objects)?);
                        let render_name = format!("{}.html", object.filename);
                        let build_path = build_dir.join(&object_def.name).join(render_name);
                        fs.write_str(&build_path, rendered)?;
                    }
                }
            }
        }
    }
    // Render regular pages
    let template_pages: HashSet<&String> = site
        .objects
        .values()
        .flat_map(|object| &object.template)
        .collect();
    let partial_re = partial_matcher();
    for file in fs.walk_dir(pages_dir, false)? {
        let file_path = pages_dir.join(&file);
        if let Some(name) = file.file_name() {
            let file_name = name.to_string_lossy();
            if file_name.ends_with(".liquid") {
                let page_name = file_name.replace(".liquid", "");
                if template_pages.contains(&page_name) || partial_re.is_match(&file_name) {
                    // template pages are not rendered as pages
                    continue;
                }
                if let Some(template_str) = fs.read_to_string(&file_path)? {
                    let page = Page::new(page_name, template_str);
                    let rendered = layout::post_process(page.render(&liquid_parser, &all_objects)?);
                    let render_name = file_name.replace(".liquid", ".html");
                    let render_path = build_dir.join(render_name);
                    debug!("rendering {}", render_path.display());
                    fs.write_str(&render_path, rendered)?;
                } else {
                    println!("template not found: {}", file.display());
                }
            }
        }
    }

    Ok(())
}

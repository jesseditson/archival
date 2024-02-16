use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{HashMap, HashSet},
    error::Error,
    path::{Path, PathBuf},
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
    pub object_definitions: ObjectDefinitions,
    pub manifest: Manifest,

    #[serde(skip)]
    obj_cache: RefCell<HashMap<PathBuf, Object>>,
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
            self.object_definitions
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

impl Site {
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
        debug!("loading definition {}", odf.display());
        let objects_table = read_toml(odf, fs)?;
        let objects = ObjectDefinition::from_table(&objects_table)?;
        Ok(Site {
            manifest,
            object_definitions: objects,
            obj_cache: RefCell::new(HashMap::new()),
        })
    }

    pub fn get_objects<T: FileSystemAPI>(
        &self,
        fs: &T,
    ) -> Result<HashMap<String, Vec<Object>>, Box<dyn Error>> {
        self.get_objects_sorted(fs, |a, b| get_order(a).partial_cmp(&get_order(b)).unwrap())
    }

    pub fn invalidate_file(&self, file: &Path) {
        println!("reload {}", file.display());
        debug!("invalidate {}", file.display());
        self.obj_cache.borrow_mut().remove(file);
    }

    pub fn get_objects_sorted<T: FileSystemAPI>(
        &self,
        fs: &T,
        sort: impl Fn(&Object, &Object) -> Ordering,
    ) -> Result<HashMap<String, Vec<Object>>, Box<dyn Error>> {
        let mut all_objects: HashMap<String, Vec<Object>> = HashMap::new();
        let objects_dir = &self.manifest.objects_dir;
        for (object_name, object_def) in self.object_definitions.iter() {
            let mut objects: Vec<Object> = Vec::new();
            let object_files_dir = objects_dir.join(object_name);
            if fs.is_dir(objects_dir)? {
                for file in fs.walk_dir(&object_files_dir, false)? {
                    if let Some(ext) = file.extension() {
                        let to_cache = if let Some(o) = self.obj_cache.borrow().get(&file) {
                            objects.push(o.clone());
                            None
                        } else if ext == "toml" {
                            debug!("parsing {} {}", object_name, file.display());
                            let obj_table = read_toml(&object_files_dir.join(&file), fs)?;
                            let o = Object::from_table(
                                object_def,
                                file.with_extension("").as_path(),
                                &obj_table,
                            )?;
                            objects.push(o.clone());
                            Some(o)
                        } else {
                            None
                        };
                        if let Some(o) = to_cache {
                            let path = object_files_dir.join(object_name).join(file);
                            debug!("cache {}", path.display());
                            self.obj_cache.borrow_mut().insert(path, o);
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

    pub fn build<T: FileSystemAPI>(&self, fs: &mut T) -> Result<(), Box<dyn Error>> {
        let objects_dir = &self.manifest.objects_dir;
        let layout_dir = &self.manifest.layout_dir;
        let pages_dir = &self.manifest.pages_dir;
        let build_dir = &self.manifest.build_dir;
        let static_dir = &self.manifest.static_dir;

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

        let all_objects = self.get_objects(fs)?;

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
        for (name, object_def) in self.object_definitions.iter() {
            if let Some(template) = &object_def.template {
                let template_path = pages_dir.join(format!("{}.liquid", template));
                debug!("rendering template objects for {}", template_path.display());
                let template_r = fs.read_to_string(&template_path);
                if template_r.is_err() {
                    println!("failed rendering {}", template_path.display());
                }
                let template_str = template_r?;
                if let Some(template_str) = template_str {
                    if let Some(t_objects) = all_objects.get(name) {
                        for object in t_objects {
                            debug!("rendering {}", object.filename);
                            let page = Page::new_with_template(
                                object.filename.clone(),
                                object_def,
                                object,
                                template_str.to_owned(),
                            );
                            let render_o = page.render(&liquid_parser, &all_objects);
                            if render_o.is_err() {
                                println!("failed rendering {}", object.filename);
                            }
                            let rendered = layout::post_process(render_o?);
                            let render_name = format!("{}.html", object.filename);
                            let t_dir = build_dir.join(&object_def.name);
                            fs.create_dir_all(&t_dir)?;
                            let build_path = t_dir.join(render_name);
                            debug!("write {}", build_path.display());
                            fs.write_str(&build_path, rendered)?;
                        }
                    }
                }
            }
        }

        // Render regular pages
        debug!("building pages in {}", pages_dir.display());
        let template_pages: HashSet<&String> = self
            .object_definitions
            .values()
            .flat_map(|object| &object.template)
            .collect();
        let partial_re = partial_matcher();
        for rel_path in fs.walk_dir(pages_dir, false)? {
            let file_path = pages_dir.join(&rel_path);
            if let Some(name) = rel_path.file_name() {
                let file_name = name.to_string_lossy();
                if file_name.ends_with(".liquid") {
                    let page_name = file_name.replace(".liquid", "");
                    let t_path = rel_path.to_string_lossy().replace(".liquid", "");
                    if template_pages.contains(&t_path) || partial_re.is_match(&file_name) {
                        // template pages are not rendered as pages
                        continue;
                    }
                    debug!("rendering {}", file_path.display());
                    if let Some(template_str) = fs.read_to_string(&file_path)? {
                        let page = Page::new(page_name, template_str);
                        let render_o = page.render(&liquid_parser, &all_objects);
                        if render_o.is_err() {
                            println!("failed rendering {}", file_path.display());
                        }
                        let rendered = layout::post_process(render_o?);
                        let mut render_dir = build_dir.to_path_buf();
                        if let Some(parent_dir) = rel_path.parent() {
                            render_dir = render_dir.join(parent_dir);
                            fs.create_dir_all(&render_dir)?;
                        }
                        let render_name = file_name.replace(".liquid", ".html");
                        let render_path = render_dir.join(render_name);
                        debug!("rendering {}", render_path.display());
                        fs.write_str(&render_path, rendered)?;
                    } else {
                        println!("page not found: {}", file_path.display());
                    }
                }
            }
        }

        Ok(())
    }
}

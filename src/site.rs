use crate::{
    check_compatibility,
    constants::MANIFEST_FILE_NAME,
    liquid_parser::{self, PARTIAL_FILE_NAME_RE},
    manifest::Manifest,
    object::{Object, ObjectEntry},
    object_definition::{ObjectDefinition, ObjectDefinitions},
    page::{Page, TemplateType},
    read_toml::read_toml,
    tags::layout,
    ArchivalError, FileSystemAPI,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{RefCell, RefMut},
    cmp::Ordering,
    collections::{HashMap, HashSet},
    error::Error,
    path::{Path, PathBuf},
};
use thiserror::Error;
use tracing::{debug, info, instrument, warn};

#[derive(Error, Debug, Clone)]
pub enum InvalidFileError {
    #[error("unrecognized file type ({0})")]
    UnrecognizedType(String),
}

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
    #[instrument(skip(fs))]
    pub fn load(fs: &impl FileSystemAPI) -> Result<Site, Box<dyn Error>> {
        // Load our manifest (should it exist)
        let manifest = match Manifest::from_file(Path::new(MANIFEST_FILE_NAME), fs) {
            Ok(m) => {
                // When loading a manifest, check its compatibility.
                if let Some(manifest_version) = &m.archival_version {
                    let (compat, message) = check_compatibility(manifest_version);
                    if !compat {
                        return Err(ArchivalError::new(&message).into());
                    }
                }
                m
            }
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
        info!("loading definition {}", odf.display());
        let objects_table = read_toml(odf, fs)?;
        let objects = ObjectDefinition::from_table(&objects_table)?;
        Ok(Site {
            manifest,
            object_definitions: objects,
            obj_cache: RefCell::new(HashMap::new()),
        })
    }

    #[instrument(skip(fs))]
    pub fn get_objects<T: FileSystemAPI>(
        &self,
        fs: &T,
    ) -> Result<HashMap<String, ObjectEntry>, Box<dyn Error>> {
        self.get_objects_sorted(fs, |a, b| get_order(a).partial_cmp(&get_order(b)).unwrap())
    }

    #[instrument]
    pub fn invalidate_file(&self, file: &Path) {
        info!("invalidate {}", file.display());
        self.obj_cache.borrow_mut().remove(file);
    }

    #[instrument(skip(fs, modify))]
    pub fn modify_manifest<T: FileSystemAPI>(
        &mut self,
        fs: &mut T,
        modify: impl FnOnce(&mut Manifest),
    ) -> Result<(), Box<dyn Error>> {
        modify(&mut self.manifest);
        fs.write_str(Path::new(MANIFEST_FILE_NAME), self.manifest.to_toml()?)
    }

    #[instrument(skip(fs, sort))]
    pub fn get_objects_sorted<T: FileSystemAPI>(
        &self,
        fs: &T,
        sort: impl Fn(&Object, &Object) -> Ordering,
    ) -> Result<HashMap<String, ObjectEntry>, Box<dyn Error>> {
        let mut all_objects: HashMap<String, ObjectEntry> = HashMap::new();
        let objects_dir = &self.manifest.objects_dir;
        for (object_name, object_def) in self.object_definitions.iter() {
            let object_files_path = objects_dir.join(object_name);
            let object_file_path = objects_dir.join(&format!("{}.toml", object_name));
            let mut cache = self.obj_cache.borrow_mut();
            if fs.is_dir(&object_files_path)? {
                if fs.exists(&object_file_path)? {
                    panic!(
                        "Cannot define both {} and {}",
                        object_files_path.to_string_lossy(),
                        object_file_path.to_string_lossy()
                    );
                }
                let mut objects: Vec<Object> = Vec::new();
                for file in fs.walk_dir(&object_files_path, false)? {
                    let path = object_files_path.join(&file);
                    if let Ok(obj) = self.object_for_path(&path, object_def, &mut cache, fs) {
                        objects.push(obj);
                    } else {
                        debug!("Invalid file {:?}", path);
                    }
                }
                // Sort objects by order key
                objects.sort_by(&sort);
                all_objects.insert(object_name.clone(), ObjectEntry::from_vec(objects));
            } else if let Ok(obj) =
                self.object_for_path(&object_file_path, object_def, &mut cache, fs)
            {
                all_objects.insert(object_name.clone(), ObjectEntry::from_object(obj));
            } else {
                warn!("failed parsing {:?}", object_file_path);
            }
        }
        Ok(all_objects)
    }

    #[instrument(skip(object_def, cache, fs))]
    fn object_for_path<T: FileSystemAPI>(
        &self,
        path: &Path,
        object_def: &ObjectDefinition,
        cache: &mut RefMut<HashMap<PathBuf, Object>>,
        fs: &T,
    ) -> Result<Object, Box<dyn Error>> {
        if path
            .extension()
            .map_or("", |e| e.to_str().map_or("", |o| o))
            != "toml"
        {
            return Err(InvalidFileError::UnrecognizedType(format!("{:?}", path)).into());
        }
        if let Some(o) = cache.get(path) {
            Ok(o.clone())
        } else {
            info!("parsing {}", path.display());
            let obj_table = read_toml(path, fs)?;
            let o = Object::from_table(
                object_def,
                Path::new(path.with_extension("").file_name().unwrap()),
                &obj_table,
            )?;
            cache.insert(path.to_path_buf(), o.clone());
            Ok(o)
        }
    }

    #[instrument(skip(fs))]
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
                info!("rendering template objects for {}", template_path.display());
                let template_r = fs.read_to_string(&template_path);
                if template_r.is_err() {
                    warn!("failed rendering {}", template_path.display());
                }
                let template_str = template_r?;
                if let Some(template_str) = template_str {
                    if let Some(t_objects) = all_objects.get(name) {
                        for object in t_objects.into_iter() {
                            info!("rendering {}", object.filename);
                            let page = Page::new_with_template(
                                object.filename.clone(),
                                object_def,
                                object,
                                template_str.to_owned(),
                                TemplateType::Default,
                                &template_path,
                            );
                            let render_o = page.render(&liquid_parser, &all_objects);
                            if render_o.is_err() {
                                warn!("failed rendering {}", object.filename);
                            }
                            let rendered = layout::post_process(render_o?);
                            let render_name = format!("{}.{}", object.filename, page.extension());
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
        info!("building pages in {}", pages_dir.display());
        let template_pages: HashSet<&str> = self
            .object_definitions
            .values()
            .flat_map(|object| object.template.as_deref())
            .collect();
        for rel_path in fs.walk_dir(pages_dir, false)? {
            let file_path = pages_dir.join(&rel_path);
            if let Some(name) = rel_path.file_name() {
                let file_name = name.to_string_lossy();
                if let Some((page_name, page_type)) = TemplateType::parse_name(&file_name) {
                    let template_path_str =
                        rel_path.with_extension("").to_string_lossy().to_string();
                    if template_pages.contains(&template_path_str[..])
                        || PARTIAL_FILE_NAME_RE.is_match(&file_name)
                    {
                        // template pages are not rendered as pages
                        continue;
                    }
                    info!(
                        "rendering {} ({})",
                        file_path.display(),
                        page_type.extension()
                    );
                    if let Some(template_str) = fs.read_to_string(&file_path)? {
                        let page = Page::new(
                            page_name.to_string(),
                            template_str,
                            TemplateType::Default,
                            &file_path,
                        );
                        let render_o = page.render(&liquid_parser, &all_objects);
                        if render_o.is_err() {
                            warn!("failed rendering {}", file_path.display());
                        }
                        let rendered = layout::post_process(render_o?);
                        let mut render_dir = build_dir.to_path_buf();
                        if let Some(parent_dir) = rel_path.parent() {
                            render_dir = render_dir.join(parent_dir);
                            fs.create_dir_all(&render_dir)?;
                        }
                        let render_path =
                            render_dir.join(format!("{}.{}", page_name, page_type.extension()));
                        debug!("write {}", render_path.display());
                        fs.write_str(&render_path, rendered)?;
                    } else {
                        warn!("page not found: {}", file_path.display());
                    }
                }
            }
        }

        Ok(())
    }
}

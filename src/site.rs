#[cfg(feature = "json-schema")]
use crate::json_schema;
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
    ArchivalError, FieldConfig, FileSystemAPI,
};
use seahash::SeaHasher;
use serde::{Deserialize, Serialize};
use std::{
    cell::{RefCell, RefMut},
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    hash::Hasher,
    path::{Path, PathBuf},
};
use thiserror::Error;
use tracing::{debug, error, instrument, trace_span, warn};

#[derive(Error, Debug, Clone)]
pub enum InvalidFileError {
    #[error("unrecognized file type ({0})")]
    UnrecognizedType(String),
    #[error("cannot define both {0} and {1}")]
    DuplicateObjectDefinition(String, String),
    #[error("invalid root object {0}: {1}")]
    InvalidRootObject(String, String),
    #[error("unknown object {0}")]
    UnknownObject(String),
}

#[derive(Error, Debug, Clone)]
pub enum BuildError {
    #[error("template file {0} does not exist.")]
    MissingTemplate(String),
    #[error("failed rendering object {0} to {1} template:\n{2}")]
    TemplateRenderError(String, String, String),
    #[error("page {0} failed rendering:\n{1}")]
    PageRenderError(String, String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Site {
    pub object_definitions: ObjectDefinitions,
    pub manifest: Manifest,

    #[serde(skip)]
    obj_cache: RefCell<HashMap<PathBuf, Object>>,
    #[serde(skip)]
    static_file_cache: RefCell<HashMap<PathBuf, u64>>,
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
        let manifest_path = Path::new(MANIFEST_FILE_NAME);
        let manifest = if fs.exists(manifest_path)? {
            let manifest = Manifest::from_file(manifest_path, fs)?;
            // When loading a manifest, check its compatibility.
            if let Some(manifest_version) = &manifest.archival_version {
                let (compat, message) = check_compatibility(manifest_version);
                if !compat {
                    return Err(ArchivalError::new(&message).into());
                }
            }
            manifest
        } else {
            Manifest::default(Path::new(""))
        };
        let odf = Path::new(&manifest.object_definition_file);
        if !fs.exists(odf)? {
            return Err(ArchivalError::new(&format!(
                "Object definition file {} does not exist",
                fs.root_dir().join(odf).to_string_lossy()
            ))
            .into());
        }

        // Load our object definitions
        #[cfg(feature = "verbose-logging")]
        debug!("loading definition {}", odf.display());
        let objects_table = read_toml(odf, fs)?;
        let objects = ObjectDefinition::from_table(&objects_table, &manifest.editor_types)?;

        Ok(Site {
            manifest,
            object_definitions: objects,
            obj_cache: RefCell::new(HashMap::new()),
            static_file_cache: RefCell::new(HashMap::new()),
        })
    }

    pub fn schema_prefix(&self) -> String {
        self.manifest.site_url.as_ref().map_or_else(
            || {
                format!(
                    "{}",
                    hash_file(
                        serde_json::to_string(&self.object_definitions)
                            .unwrap_or_default()
                            .as_bytes()
                    )
                )
            },
            |s| s.to_owned(),
        )
    }

    #[cfg(feature = "json-schema")]
    pub fn root_objects(&self, fs: &impl FileSystemAPI) -> HashSet<String> {
        let mut root_objects = HashSet::new();
        let objects_dir = &self.manifest.objects_dir;
        for object_name in self.object_definitions.keys() {
            let root_object_file_path = objects_dir.join(format!("{}.toml", object_name));
            if fs.exists(&root_object_file_path).unwrap() {
                root_objects.insert(object_name.to_string());
            }
        }
        root_objects
    }

    #[cfg(feature = "json-schema")]
    pub fn dump_schemas(&self, fs: &mut impl FileSystemAPI) -> Result<(), Box<dyn Error>> {
        use crate::ObjectSchemaOptions;

        debug!(
            "dumping schemas for {}",
            self.manifest.object_definition_file.display()
        );
        let _ = fs.remove_dir_all(&self.manifest.schemas_dir);
        fs.create_dir_all(&self.manifest.schemas_dir)?;
        for (name, def) in &self.object_definitions {
            let schema = json_schema::generate_json_schema(
                &format!("{}/{}.schema.json", self.schema_prefix(), name),
                def,
                ObjectSchemaOptions::default(),
            );
            fs.write_str(
                &self
                    .manifest
                    .schemas_dir
                    .join(format!("{}.schema.json", name)),
                serde_json::to_string_pretty(&schema).unwrap(),
            )?;
        }
        Ok(())
    }
    #[cfg(feature = "json-schema")]
    pub fn dump_schema(
        &self,
        object: &String,
        fs: &mut impl FileSystemAPI,
    ) -> Result<(), Box<dyn Error>> {
        use crate::ObjectSchemaOptions;

        debug!("dumping schema for {}", object);
        let def = self
            .object_definitions
            .get(object)
            .ok_or_else(|| InvalidFileError::UnknownObject(object.clone()))?;
        let schema = json_schema::generate_json_schema(
            &format!("{}/{}.schema.json", self.schema_prefix(), object),
            def,
            ObjectSchemaOptions::default(),
        );
        fs.write_str(
            &self
                .manifest
                .schemas_dir
                .join(format!("{}.schema.json", object)),
            serde_json::to_string_pretty(&schema).unwrap(),
        )?;
        Ok(())
    }

    pub fn get_field_config(&self) -> FieldConfig {
        FieldConfig::new(self.manifest.uploads_url.as_ref().map(|u| u.to_owned()))
    }

    #[instrument(skip(fs))]
    pub fn get_objects<T: FileSystemAPI>(
        &self,
        fs: &T,
    ) -> Result<BTreeMap<String, ObjectEntry>, Box<dyn Error>> {
        self.get_objects_sorted(
            fs,
            Some(|a: &_, b: &_| get_order(a).partial_cmp(&get_order(b)).unwrap()),
        )
    }

    #[instrument]
    pub fn invalidate_file(&self, file: &Path) {
        #[cfg(feature = "verbose-logging")]
        debug!("invalidate {}", file.display());
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

    pub fn manifest_content<T: FileSystemAPI>(&self, fs: &T) -> Result<String, Box<dyn Error>> {
        fs.read_to_string(Path::new(MANIFEST_FILE_NAME))
            .map(|m| m.unwrap_or_default())
    }

    #[instrument(skip(fs))]
    pub fn get_object<T: FileSystemAPI>(
        &self,
        object_name: &str,
        filename: Option<&str>,
        fs: &T,
    ) -> Result<Object, Box<dyn Error>> {
        let object_def = self
            .object_definitions
            .get(object_name)
            .ok_or_else(|| InvalidFileError::UnknownObject(object_name.to_string()))?;
        let path = self.path_for_object(object_name, filename);
        let mut cache = self.obj_cache.borrow_mut();
        self.object_for_path(&path, object_def, &mut cache, fs)
    }

    fn path_for_object(&self, object_name: &str, filename: Option<&str>) -> PathBuf {
        let objects_dir = &self.manifest.objects_dir;
        if let Some(filename) = filename {
            objects_dir.join(object_name).join(filename)
        } else {
            objects_dir.join(format!("{}.toml", object_name))
        }
    }

    #[instrument(skip(fs, sort))]
    pub fn get_objects_sorted<T: FileSystemAPI>(
        &self,
        fs: &T,
        sort: Option<impl Fn(&Object, &Object) -> Ordering>,
    ) -> Result<BTreeMap<String, ObjectEntry>, Box<dyn Error>> {
        let mut all_objects: BTreeMap<String, ObjectEntry> = BTreeMap::new();
        let objects_dir = &self.manifest.objects_dir;
        for (object_name, object_def) in self.object_definitions.iter() {
            let object_files_path = objects_dir.join(object_name);
            let object_file_path = objects_dir.join(format!("{}.toml", object_name));
            let mut cache = self.obj_cache.borrow_mut();
            if fs.is_dir(&object_files_path)? {
                if fs.exists(&object_file_path)? {
                    return Err(InvalidFileError::DuplicateObjectDefinition(
                        object_files_path.display().to_string(),
                        object_file_path.display().to_string(),
                    )
                    .into());
                }
                let mut objects: Vec<Object> = Vec::new();
                for file in fs.walk_dir(&object_files_path, false)? {
                    let path = object_files_path.join(&file);
                    match self.object_for_path(&path, object_def, &mut cache, fs) {
                        Ok(obj) => {
                            objects.push(obj);
                        }
                        Err(err) => {
                            println!("Invalid file {:?}: {}", path, err);
                            error!("Invalid file {:?}: {}", path, err);
                        }
                    }
                }
                // Sort objects by order key
                if let Some(sort) = &sort {
                    trace_span!("sort objects");
                    objects.sort_by(sort);
                }
                all_objects.insert(object_name.clone(), ObjectEntry::from_vec(objects));
            } else if fs.exists(&object_file_path)? {
                match self.object_for_path(&object_file_path, object_def, &mut cache, fs) {
                    Ok(obj) => {
                        all_objects.insert(object_name.clone(), ObjectEntry::from_object(obj));
                    }
                    Err(error) => {
                        // This error is unrecoverable because if we have a root
                        // file, we cannot create an empty list for this type
                        // since it would violate our "list or root" rule.
                        return Err(InvalidFileError::InvalidRootObject(
                            object_file_path.display().to_string(),
                            error.to_string(),
                        )
                        .into());
                    }
                }
            } else {
                all_objects.insert(object_name.clone(), ObjectEntry::empty_list());
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
            #[cfg(feature = "verbose-logging")]
            debug!("parsing {}", path.display());
            let obj_table = read_toml(path, fs)?;
            let o = Object::from_table(
                object_def,
                Path::new(path.with_extension("").file_name().unwrap()),
                &obj_table,
                &self.manifest.editor_types,
                // Skip validation when populating the cache, we may have new
                // objects with invalid unset keys
                true,
            )?;
            cache.insert(path.to_path_buf(), o.clone());
            Ok(o)
        }
    }

    #[instrument(skip(fs))]
    pub fn sync_static_files<T: FileSystemAPI>(&self, fs: &mut T) -> Result<(), Box<dyn Error>> {
        let Manifest {
            static_dir,
            build_dir,
            ..
        } = &self.manifest;
        if !fs.exists(build_dir)? {
            fs.create_dir_all(build_dir)?;
        }
        let mut hashes = self.static_file_cache.borrow_mut();
        let last_dist_paths: Vec<PathBuf> = hashes.keys().cloned().collect();
        let mut copied_paths: HashSet<PathBuf> = HashSet::new();
        // Copy static files
        #[cfg(feature = "verbose-logging")]
        debug!("copying files from {}", static_dir.display());
        if fs.exists(static_dir)? {
            for file in fs.walk_dir(static_dir, false)? {
                let from = static_dir.join(&file);
                if let Some(content) = fs.read(&from)? {
                    let current_hash = hash_file(&content);
                    copied_paths.insert(file.clone());
                    // If there is an existing hash and it matches the current
                    // file, leave it there.
                    if let Some(existing_hash) = hashes.get(&file) {
                        if *existing_hash == current_hash {
                            continue;
                        }
                    }
                    // Otherwise, copy the file and store the latest hash.
                    let dest = build_dir.join(&file);
                    if let Some(dirname) = dest.parent() {
                        if dirname != build_dir {
                            fs.create_dir_all(dirname)?;
                        }
                    }
                    fs.write(&dest, content)?;
                    hashes.insert(file, current_hash);
                }
            }
            // Remove any files in dest that are no longer in static
            for path in last_dist_paths {
                if !copied_paths.contains(&path) {
                    fs.delete(&path)?;
                    hashes.remove(&path);
                }
            }
        } else {
            debug!("static dir {} does not exist.", static_dir.display());
        }
        Ok(())
    }

    #[instrument(skip(fs))]
    pub fn build<T: FileSystemAPI>(&self, fs: &mut T) -> Result<(), Box<dyn Error>> {
        let Manifest {
            objects_dir,
            layout_dir,
            pages_dir,
            build_dir,
            ..
        } = &self.manifest;

        // Validate paths
        if !fs.exists(objects_dir)? {
            return Err(ArchivalError::new(&format!(
                "Objects dir {} does not exist",
                fs.root_dir().join(objects_dir).to_string_lossy()
            ))
            .into());
        }
        if !fs.exists(pages_dir)? {
            return Err(ArchivalError::new(&format!(
                "Pages dir {} does not exist",
                fs.root_dir().join(pages_dir).to_string_lossy()
            ))
            .into());
        }
        if !fs.exists(build_dir)? {
            fs.create_dir_all(build_dir)?;
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
                #[cfg(feature = "verbose-logging")]
                debug!("rendering template objects for {}", template_path.display());
                if !fs.exists(&template_path)? {
                    return Err(
                        BuildError::MissingTemplate(template_path.display().to_string()).into(),
                    );
                }
                let template_r = fs.read_to_string(&template_path);
                if template_r.is_err() {
                    warn!("failed rendering {}", template_path.display());
                }
                let template_str = template_r?;
                if let Some(template_str) = template_str {
                    if let Some(t_objects) = all_objects.get(name) {
                        for object in t_objects.into_iter() {
                            #[cfg(feature = "verbose-logging")]
                            debug!("rendering {}", object.filename);
                            if let Err(error) = Self::render_template_page(
                                object,
                                object_def,
                                &template_str,
                                &template_path,
                                build_dir,
                                &all_objects,
                                fs,
                                &liquid_parser,
                            ) {
                                return Err(BuildError::TemplateRenderError(
                                    object.filename.to_string(),
                                    template.to_string(),
                                    error.to_string(),
                                )
                                .into());
                            }
                        }
                    }
                }
            }
        }

        // Render regular pages
        #[cfg(feature = "verbose-logging")]
        debug!("building pages in {}", pages_dir.display());
        let template_pages: HashSet<&str> = self
            .object_definitions
            .values()
            .flat_map(|object| object.template.as_deref())
            .collect();
        for rel_path in fs.walk_dir(pages_dir, false)? {
            let file_path = pages_dir.join(&rel_path);
            if let Some(name) = rel_path.file_name() {
                let file_name = name.to_string_lossy();
                if let Some((page_name, page_type)) = TemplateType::parse_path(&file_name) {
                    let template_path_str =
                        rel_path.with_extension("").to_string_lossy().to_string();
                    if template_pages.contains(&template_path_str[..])
                        || PARTIAL_FILE_NAME_RE.is_match(&file_name)
                    {
                        // template pages are not rendered as pages
                        continue;
                    }
                    #[cfg(feature = "verbose-logging")]
                    debug!(
                        "rendering {} ({})",
                        file_path.display(),
                        page_type.extension()
                    );
                    if let Err(error) = Self::render_page(
                        &rel_path,
                        &file_path,
                        page_name,
                        page_type,
                        build_dir,
                        &all_objects,
                        fs,
                        &liquid_parser,
                    ) {
                        return Err(BuildError::PageRenderError(
                            page_name.to_string(),
                            error.to_string(),
                        )
                        .into());
                    }
                }
            }
        }
        Ok(())
    }

    #[instrument(skip(all_objects, fs, liquid_parser))]
    #[allow(clippy::too_many_arguments)]
    fn render_template_page<T: FileSystemAPI>(
        object: &Object,
        object_def: &ObjectDefinition,
        template_str: &String,
        template_path: &PathBuf,
        build_dir: &PathBuf,
        all_objects: &BTreeMap<String, ObjectEntry>,
        fs: &mut T,
        liquid_parser: &liquid::Parser,
    ) -> Result<(), Box<dyn Error>> {
        let page = Page::new_with_template(
            object.filename.clone(),
            object_def,
            object,
            template_str.to_owned(),
            TemplateType::Default,
            template_path,
        );
        let render_o = page.render(liquid_parser, all_objects);
        if render_o.is_err() {
            warn!("failed rendering {}", object.filename);
        }
        let rendered = layout::post_process(render_o?);
        let render_name = format!("{}.{}", object.filename, page.extension());
        let t_dir = build_dir.join(&object_def.name);
        fs.create_dir_all(&t_dir)?;
        let build_path = t_dir.join(render_name);
        #[cfg(feature = "verbose-logging")]
        debug!("write {}", build_path.display());
        fs.write_str(&build_path, rendered)?;
        Ok(())
    }

    #[instrument(skip(all_objects, fs, liquid_parser))]
    #[allow(clippy::too_many_arguments)]
    fn render_page<T: FileSystemAPI>(
        rel_path: &PathBuf,
        file_path: &PathBuf,
        page_name: &str,
        page_type: TemplateType,
        build_dir: &PathBuf,
        all_objects: &BTreeMap<String, ObjectEntry>,
        fs: &mut T,
        liquid_parser: &liquid::Parser,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(template_str) = fs.read_to_string(file_path)? {
            let page = Page::new(
                page_name.to_string(),
                template_str,
                TemplateType::Default,
                file_path,
            );
            let render_o = page.render(liquid_parser, all_objects);
            if render_o.is_err() {
                warn!("failed rendering {}", file_path.display());
            }
            let rendered = layout::post_process(render_o?);
            let mut render_dir = build_dir.to_path_buf();
            if let Some(parent_dir) = rel_path.parent() {
                render_dir = render_dir.join(parent_dir);
                fs.create_dir_all(&render_dir)?;
            }
            let render_path = render_dir.join(format!("{}.{}", page_name, page_type.extension()));
            #[cfg(feature = "verbose-logging")]
            debug!("write {}", render_path.display());
            fs.write_str(&render_path, rendered)?;
        } else {
            warn!("page not found: {}", file_path.display());
        }
        Ok(())
    }
}

fn hash_file(file: &[u8]) -> u64 {
    let mut hasher = SeaHasher::new();
    hasher.write(file);
    hasher.finish()
}

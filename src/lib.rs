use std::{
    collections::HashMap,
    env,
    error::Error,
    path::{Path, PathBuf},
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

mod field_value;
mod file_system;
mod file_system_mutex;
mod filters;
mod liquid_parser;
mod manifest;
mod object;
mod object_definition;
mod page;
mod read_toml;
mod reserved_fields;
mod tags;

use constants::MANIFEST_FILE_NAME;
#[cfg(feature = "stdlib-fs")]
use ctrlc;
pub use file_system::{FileSystemAPI, WatchableFileSystemAPI};
use file_system_mutex::FileSystemMutex;
use manifest::Manifest;
use object::Object;
use object_definition::{ObjectDefinition, ObjectDefinitions};
use page::Page;
use read_toml::read_toml;
use serde::{Deserialize, Serialize};
use tags::layout;

mod constants;
#[cfg(feature = "stdlib-fs")]
mod file_system_stdlib;
#[cfg(feature = "wasm-fs")]
mod file_system_wasm;

#[cfg(feature = "wasm-fs")]
pub use file_system_wasm::WasmFileSystem;

#[derive(Debug)]
struct ArchivalError {
    message: String,
}
impl ArchivalError {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_owned(),
        }
    }
}
impl std::fmt::Display for ArchivalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Archival Error: {}", self.message)
    }
}
impl Error for ArchivalError {
    fn description(&self) -> &str {
        &self.message
    }
}

static INVALID_COMMAND: &str = "Valid commands are `build` and `run`.";

#[cfg(feature = "stdlib-fs")]
pub fn binary(mut args: impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let mut build_dir = env::current_dir()?;
    let _bin_name = args.next();
    if let Some(command_arg) = args.next() {
        let path_arg = args.next();
        if let Some(path) = path_arg {
            build_dir = build_dir.join(path);
        }
        let fs_a = FileSystemMutex::init(file_system_stdlib::NativeFileSystem);

        let site = fs_a.with_fs(|fs| load_site(&build_dir, fs))?;
        match &command_arg[..] {
            "build" => {
                println!("Building site: {}", &site);
                build_site(&site, fs_a)?;
            }
            "run" => {
                println!("Watching site: {}", &site);
                fs_a.clone().with_fs(|fs| {
                    // This won't leak because the process is ended when we
                    // abort anyway
                    _ = fs.watch(
                        site.root.to_owned(),
                        site.manifest.watched_paths(),
                        move |paths| {
                            println!("Changed: {:?}", paths);
                            build_site(&site, fs_a.clone()).unwrap_or_else(|err| {
                                println!("Failed reloading site: {}", err);
                            })
                        },
                    )?;
                    Ok(())
                })?;
                let aborted = Arc::new(AtomicBool::new(false));
                let aborted_clone = aborted.clone();
                ctrlc::set_handler(move || {
                    aborted_clone.store(true, Ordering::SeqCst);
                })?;
                loop {
                    if aborted.load(Ordering::SeqCst) {
                        exit(0);
                    }
                }
            }
            _ => {
                return Err(ArchivalError::new(INVALID_COMMAND).into());
            }
        }
    } else {
        return Err(ArchivalError::new(INVALID_COMMAND).into());
    }
    Ok(())
}

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
                .map(|o| format!("{}", o.as_str()))
                .collect::<Vec<String>>()
                .join("\n"),
            self.manifest
        )
    }
}

pub fn load_site(root: &Path, fs: &impl FileSystemAPI) -> Result<Site, Box<dyn Error>> {
    // Load our manifest (should it exist)
    let manifest = match Manifest::from_file(&root.join(MANIFEST_FILE_NAME), fs) {
        Ok(m) => m,
        Err(_) => Manifest::default(root),
    };
    let odf = root.join(&manifest.object_definition_file);
    if !odf.exists() {
        return Err(ArchivalError::new(&format!(
            "Object definition file {} does not exist",
            odf.to_string_lossy()
        ))
        .into());
    }

    // Load our object definitions
    let objects_table = read_toml(&odf)?;
    let objects = ObjectDefinition::from_table(&objects_table)?;
    Ok(Site {
        root: root.to_path_buf(),
        manifest,
        objects,
    })
}

pub fn build_site<T: FileSystemAPI>(
    site: &Site,
    fs: FileSystemMutex<T>,
) -> Result<(), Box<dyn Error>> {
    let mut all_objects: HashMap<String, Vec<Object>> = HashMap::new();
    let objects_dir = site.root.join(&site.manifest.objects_dir);
    let layout_dir = site.root.join(&site.manifest.layout_dir);
    let pages_dir = site.root.join(&site.manifest.pages_dir);
    let build_dir = site.root.join(&site.manifest.build_dir);
    let static_dir = site.root.join(&site.manifest.static_dir);

    // Validate paths
    if !objects_dir.exists() {
        return Err(ArchivalError::new(&format!(
            "Objects dir {} does not exist",
            objects_dir.to_string_lossy()
        ))
        .into());
    }
    if !pages_dir.exists() {
        return Err(ArchivalError::new(&format!(
            "Pages dir {} does not exist",
            pages_dir.to_string_lossy()
        ))
        .into());
    }
    if !build_dir.exists() {
        fs.with_fs(|f| f.create_dir_all(&build_dir))?;
    }

    // Copy static files
    if static_dir.exists() {
        fs.clone()
            .with_fs(|f| f.copy_contents(&static_dir, &build_dir))?;
    }

    for (object_name, object_def) in site.objects.iter() {
        let mut objects: Vec<Object> = Vec::new();
        let object_files_dir = objects_dir.join(object_name);
        if objects_dir.is_dir() {
            for file in fs.with_fs(|f| f.read_dir(&object_files_dir))? {
                if file.ends_with(".toml") {
                    let obj_table = read_toml(&file)?;
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
                        fs.with_fs(|f| f.write(&build_path, rendered))?;
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
                if let Some(template_str) = fs.with_fs(|f| f.read_to_string(&file.as_path()))? {
                    let page = Page::new(page_name, template_str);
                    let rendered = layout::post_process(page.render(&liquid_parser, &all_objects)?);
                    let render_name = file_name.replace(".liquid", ".html");
                    fs.with_fs(|f| f.write(&build_dir.join(render_name), rendered))?;
                } else {
                    println!("template not found: {}", file.display());
                }
            }
        }
    }

    Ok(())
}

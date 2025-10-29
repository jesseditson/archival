mod archival_error;
mod file_system;
mod file_system_memory;
mod file_system_mutex;
#[cfg(test)]
mod file_system_tests;
mod filters;
mod liquid_parser;
pub mod manifest;
mod object_definition;
mod page;
mod read_toml;
mod reserved_fields;
mod site;
mod tags;
#[cfg(test)]
mod test_utils;
mod util;
mod value_path;
pub use constants::{MANIFEST_FILE_NAME, MIN_COMPAT_VERSION};
use events::{
    AddChildEvent, AddObjectEvent, ArchivalEvent, DeleteObjectEvent, EditFieldEvent,
    EditOrderEvent, RemoveChildEvent, RenameObjectEvent,
};
use events::{AddRootObjectEvent, ArchivalEventResponse};
pub use fields::FieldConfig;
pub use fields::FieldValue;
use manifest::Manifest;
use mime_guess::MimeGuess;
use seahash::SeaHasher;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use site::Site;
use std::cmp::Ordering;
use std::error::Error;
use std::fmt::Debug;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use tracing::{debug, error};
#[cfg(feature = "binary")]
pub mod binary;
mod constants;
#[cfg(feature = "stdlib-fs")]
mod file_system_stdlib;
#[cfg(feature = "json-schema")]
mod json_schema;
#[cfg(feature = "binary")]
mod server;
use file_system_mutex::FileSystemMutex;
use object::{Object, ObjectEntry};
use semver::{Version, VersionReq};

// Re-exports
pub mod events;
pub mod fields;
pub mod object;
pub use archival_error::ArchivalError;
pub use file_system::unpack_zip;
pub use file_system::FileSystemAPI;
pub use file_system_memory::MemoryFileSystem;
#[cfg(feature = "json-schema")]
pub use json_schema::{ObjectSchema, ObjectSchemaOptions};
pub use object::ObjectMap;
pub use object_definition::{ObjectDefinition, ObjectDefinitions};

use crate::fields::FieldType;
use crate::object::ValuePath;

pub type ArchivalBuildId = u64;

#[cfg(feature = "typescript")]
pub mod typedefs {
    pub use crate::object_definition::typedefs::*;
}

#[derive(Debug, Default)]
pub struct BuildOptions {
    pub skip_static: bool,
}

impl BuildOptions {
    pub fn no_static() -> Self {
        Self { skip_static: true }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DistFile {
    pub path: PathBuf,
    pub mime: String,
    pub data: Vec<u8>,
}
impl DistFile {
    fn new(path: PathBuf, data: Vec<u8>) -> Self {
        Self {
            mime: MimeGuess::from_path(&path)
                .first_or_octet_stream()
                .essence_str()
                .to_string(),
            data,
            path,
        }
    }
}

pub static ARCHIVAL_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) fn check_compatibility(version_string: &str) -> (bool, String) {
    let req = VersionReq::parse(MIN_COMPAT_VERSION).unwrap();
    match Version::parse(version_string) {
        Ok(version) => {
            if req.matches(&version) {
                (true, "passed compatibility check.".to_owned())
            } else {
                (false, format!("site archival version {} is incompatible with this version of archival (minimum required version {}).", version, MIN_COMPAT_VERSION))
            }
        }
        Err(e) => (false, format!("invalid version {}: {}", version_string, e)),
    }
}

#[derive(Debug)]
pub struct Archival<F: FileSystemAPI + Clone + Debug> {
    fs_mutex: FileSystemMutex<F>,
    pub site: site::Site,
    last_build_id: AtomicU64,
}

impl<F: FileSystemAPI + Clone + Debug> Archival<F> {
    pub fn is_compatible(fs: &F) -> Result<bool, Box<dyn Error>> {
        let site = Site::load(fs, Some(""))?;
        if let Some(version_str) = &site.manifest.archival_version {
            let (ok, msg) = check_compatibility(version_str);
            if !ok {
                error!("incompatible: {}", msg);
            }
            Ok(ok)
        } else {
            Ok(true)
        }
    }
    pub fn new(fs: F) -> Result<Self, Box<dyn Error>> {
        let site = Site::load(&fs, None)?;
        FieldConfig::set_global(site.get_field_config(None)?);
        let fs_mutex = FileSystemMutex::init(fs);
        Ok(Self {
            fs_mutex,
            site,
            last_build_id: AtomicU64::new(0),
        })
    }
    pub fn new_with_upload_prefix(fs: F, upload_prefix: &str) -> Result<Self, Box<dyn Error>> {
        let site = Site::load(&fs, Some(upload_prefix))?;
        FieldConfig::set_global(site.get_field_config(Some(upload_prefix))?);
        let fs_mutex = FileSystemMutex::init(fs);
        Ok(Self {
            fs_mutex,
            site,
            last_build_id: AtomicU64::new(0),
        })
    }
    pub fn new_with_field_config(fs: F, field_config: FieldConfig) -> Result<Self, Box<dyn Error>> {
        let site = Site::load(&fs, Some(&field_config.upload_prefix))?;
        FieldConfig::set_global(field_config);
        let fs_mutex = FileSystemMutex::init(fs);
        Ok(Self {
            fs_mutex,
            site,
            last_build_id: AtomicU64::new(0),
        })
    }
    pub fn build(&self, options: BuildOptions) -> Result<ArchivalBuildId, Box<dyn Error>> {
        let build_id = self.fs_mutex.with_fs(|fs| {
            if !options.skip_static {
                self.site.sync_static_files(fs)?;
            }
            let build_id = self.site.build_id();
            if build_id == 0 || self.last_build_id.load(AtomicOrdering::Relaxed) != build_id {
                debug!("build {} {:#?}", self.site, options);
                self.site.build(fs)?;
            } else {
                #[cfg(feature = "verbose-logging")]
                debug!("skipping duplicate build");
            }
            Ok(build_id)
        })?;
        self.last_build_id
            .fetch_update(AtomicOrdering::Relaxed, AtomicOrdering::Relaxed, |_| {
                Some(build_id)
            })
            .unwrap();
        Ok(self.last_build_id.load(AtomicOrdering::Relaxed))
    }
    #[cfg(feature = "json-schema")]
    pub fn dump_schemas(&self) -> Result<(), Box<dyn Error>> {
        debug!("dump schemas {}", self.site);
        self.fs_mutex.with_fs(|fs| self.site.dump_schemas(fs))
    }
    #[cfg(feature = "json-schema")]
    pub fn generate_root_json_schema(&self, options: ObjectSchemaOptions) -> ObjectSchema {
        json_schema::generate_root_json_schema(
            &format!("{}/root.schema.json", self.site.schema_prefix()),
            self.site.manifest.site_name.as_deref(),
            &format!(
                "Object definitions{}",
                if let Some(name) = options
                    .name
                    .as_ref()
                    .and(self.site.manifest.site_name.as_ref())
                    .to_owned()
                {
                    format!(" for {}", name)
                } else {
                    "".to_string()
                }
            ),
            &self.site.object_definitions,
            &self
                .fs_mutex
                .with_fs(|fs| Ok(self.site.root_objects(fs)))
                .unwrap(),
            options,
        )
    }
    pub fn dist_file(&self, path: &Path) -> Option<Vec<u8>> {
        let path = self.site.manifest.build_dir.join(path);
        self.fs_mutex.with_fs(|fs| fs.read(&path)).unwrap_or(None)
    }
    pub fn dist_files(&self) -> Vec<DistFile> {
        let mut files = vec![];
        self.fs_mutex
            .with_fs(|fs| {
                let build_dir = &self.site.manifest.build_dir;
                for file in fs.walk_dir(build_dir, true)? {
                    if let Some(data) = fs.read(build_dir.join(&file)).unwrap_or(None) {
                        files.push(DistFile::new(file, data));
                    }
                }
                Ok(())
            })
            .unwrap();
        files
    }
    pub fn object_exists(&self, obj_type: &str, filename: &str) -> Result<bool, Box<dyn Error>> {
        self.fs_mutex
            .with_fs(|fs| fs.exists(&self.object_path_impl(obj_type, filename, fs)?))
    }
    pub fn object_path(&self, obj_type: &str, filename: &str) -> PathBuf {
        self.fs_mutex
            .with_fs(|fs| Ok(self.object_path_impl(obj_type, filename, fs).unwrap()))
            .unwrap()
    }
    pub fn build_id(&self) -> u64 {
        self.site.build_id()
    }
    pub fn fs_id(&self) -> Result<u64, Box<dyn Error>> {
        self.fs_mutex.with_fs(|fs| self.fs_id_for_fs(fs))
    }
    pub fn list_build_files(
        &self,
    ) -> Result<impl Iterator<Item = PathBuf> + use<'_, F>, Box<dyn Error>> {
        self.fs_mutex.with_fs(|fs| self.list_build_files_for_fs(fs))
    }
    fn list_build_files_for_fs(
        &self,
        fs: &F,
    ) -> Result<impl Iterator<Item = PathBuf> + use<'_, F>, Box<dyn Error>> {
        let Manifest {
            object_definition_file,
            pages_dir,
            layout_dir,
            objects_dir,
            ..
        } = &self.site.manifest;
        let root_files = [
            Path::new(MANIFEST_FILE_NAME).to_path_buf(),
            object_definition_file.to_owned(),
        ];
        Ok(root_files
            .into_iter()
            .chain(fs.walk_dir(pages_dir, false)?.map(|p| pages_dir.join(p)))
            .chain(fs.walk_dir(layout_dir, false)?.map(|p| layout_dir.join(p)))
            .chain(
                fs.walk_dir(objects_dir, false)?
                    .map(|p| objects_dir.join(p)),
            ))
    }
    fn fs_id_for_fs(&self, fs: &F) -> Result<u64, Box<dyn Error>> {
        let mut hasher = SeaHasher::new();
        for path in self.list_build_files_for_fs(fs)? {
            if let Some(file) = fs.read(&path)? {
                hasher.write(&file);
            } else {
                debug!("no content found for {}", path.display());
            }
        }
        Ok(hasher.finish())
    }
    fn object_path_impl(
        &self,
        obj_type: &str,
        filename: &str,
        fs: &F,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let objects = self.site.get_objects(fs)?;
        let entry = objects.get(obj_type).ok_or(ArchivalError::new(&format!(
            "object type not found: {}",
            obj_type
        )))?;
        Ok(if matches!(entry, ObjectEntry::Object(_)) {
            self.site
                .manifest
                .objects_dir
                .join(Path::new(&format!("{}.toml", obj_type)))
        } else {
            self.site
                .manifest
                .objects_dir
                .join(Path::new(&obj_type))
                .join(Path::new(&format!("{}.toml", filename)))
        })
    }
    pub fn object_file(&self, obj_type: &str, filename: &str) -> Result<String, Box<dyn Error>> {
        self.fs_mutex
            .with_fs(|fs| self.modify_object_file(obj_type, filename, |o| Ok(o), fs))
    }
    pub fn sha_for_file(&self, file: &Path) -> Result<String, Box<dyn Error>> {
        let file_data = self
            .fs_mutex
            .with_fs(|fs| fs.read(file))?
            .ok_or_else(|| ArchivalError::new("failed generating sha"))?;
        let mut hasher = Sha256::new();
        // write input message
        hasher.update(&file_data[..]);
        Ok(data_encoding::HEXLOWER.encode(&hasher.finalize()))
    }

    pub fn write_file(
        &self,
        obj_type: &str,
        filename: &str,
        contents: String,
    ) -> Result<(), Box<dyn Error>> {
        // Validate toml
        let obj_def = self
            .site
            .object_definitions
            .get(obj_type)
            .ok_or(ArchivalError::new(&format!(
                "object not found: {}",
                obj_type
            )))?;
        let table: toml::Table = toml::from_str(&contents)?;
        // Note that this also fails when custom validation fails.
        let _ = Object::from_table(
            obj_def,
            Path::new(filename),
            &table,
            &self.site.manifest.editor_types,
            false,
        )?;
        // Object is valid, write it
        self.fs_mutex
            .with_fs(|fs| fs.write_str(&self.object_path_impl(obj_type, filename, fs)?, contents))
    }
    fn modify_object_file(
        &self,
        obj_type: &str,
        filename: &str,
        obj_cb: impl FnOnce(&mut Object) -> Result<&mut Object, Box<dyn Error>>,
        fs: &F,
    ) -> Result<String, Box<dyn Error>> {
        let mut all_objects = self.site.get_objects(fs)?;
        if let Some(objects) = all_objects.get_mut(obj_type) {
            if let Some(object) = objects.iter_mut().find(|o| o.filename == filename) {
                let object = obj_cb(object)?;
                Ok(object.to_toml()?)
            } else {
                Err(ArchivalError::new(&format!("filename not found: {}", filename)).into())
            }
        } else {
            Err(ArchivalError::new(&format!("no objects of type: {}", obj_type)).into())
        }
    }

    pub fn send_event(
        &self,
        event: ArchivalEvent,
        build_options: Option<BuildOptions>,
    ) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        let r = match event {
            ArchivalEvent::AddObject(event) => self.add_object(event)?,
            ArchivalEvent::RenameObject(event) => self.rename_object(event)?,
            ArchivalEvent::AddRootObject(event) => self.add_root_object(event)?,
            ArchivalEvent::DeleteObject(event) => self.delete_object(event)?,
            ArchivalEvent::EditField(event) => self.edit_field(event)?,
            ArchivalEvent::EditOrder(event) => self.edit_order(event)?,
            ArchivalEvent::AddChild(event) => self.add_child(event)?,
            ArchivalEvent::RemoveChild(event) => self.remove_child(event)?,
        };
        if let Some(build_options) = build_options {
            self.build(build_options)?;
        }
        Ok(r)
    }

    // Internal
    fn add_root_object(
        &self,
        event: AddRootObjectEvent,
    ) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        let obj_def = self
            .site
            .object_definitions
            .get(&event.object)
            .ok_or(ArchivalError::new(&format!(
                "object not found: {}",
                event.object
            )))?;
        self.fs_mutex.with_fs(|fs| {
            let dir_path = self
                .site
                .manifest
                .objects_dir
                .join(Path::new(&event.object));
            if fs.is_dir(&dir_path)? && fs.walk_dir(&dir_path, false)?.next().is_some() {
                return Err(ArchivalError::new(&format!(
                    "cannod add root {} object, found existing non-roots.",
                    event.object
                ))
                .into());
            }
            let path = self
                .site
                .manifest
                .objects_dir
                .join(Path::new(&format!("{}.toml", event.object)));
            if fs.exists(&path)? {
                return Err(ArchivalError::new(&format!(
                    "cannod add root {}, file already exists.",
                    event.object
                ))
                .into());
            }
            let object = Object::from_def(obj_def, &event.object, None, event.values)?;
            fs.write_str(&path, object.to_toml()?)?;
            self.site.invalidate_file(&path);
            Ok(())
        })?;
        Ok(ArchivalEventResponse::None)
    }

    fn add_object(&self, event: AddObjectEvent) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        let obj_def = self
            .site
            .object_definitions
            .get(&event.object)
            .ok_or(ArchivalError::new(&format!(
                "object not found: {}",
                event.object
            )))?;
        self.fs_mutex.with_fs(|fs| {
            let obj_dir = self
                .site
                .manifest
                .objects_dir
                .join(Path::new(&event.object));
            fs.create_dir_all(&obj_dir)?;
            let path = obj_dir.join(Path::new(&format!("{}.toml", event.filename)));
            if fs.exists(&path)? {
                return Err(ArchivalError::new(&format!(
                    "cannod add {} named {}, file already exists.",
                    event.object, event.filename
                ))
                .into());
            }
            let root_path = self
                .site
                .manifest
                .objects_dir
                .join(Path::new(&format!("{}.toml", event.object)));
            if fs.exists(&root_path)? {
                return Err(ArchivalError::new(&format!(
                    "cannod add {} named {}, there's already a root {}.",
                    event.object, event.filename, event.object
                ))
                .into());
            }
            let object = Object::from_def(obj_def, &event.filename, event.order, event.values)?;
            fs.write_str(&path, object.to_toml()?).map_err(|error| {
                ArchivalError::new(&format!("failed writing to {}: {}", path.display(), error))
            })?;
            self.site.invalidate_file(&path);
            Ok(())
        })?;
        Ok(ArchivalEventResponse::None)
    }

    fn rename_object(
        &self,
        event: RenameObjectEvent,
    ) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        let obj_def = self
            .site
            .object_definitions
            .get(&event.object)
            .ok_or(ArchivalError::new(&format!(
                "object not found: {}",
                event.object
            )))?;
        self.fs_mutex.with_fs(|fs| {
            let root_objects = self.site.root_objects(fs);
            if root_objects.contains(&event.from) {
                return Err(ArchivalError::new(&format!(
                    "cannot rename root object {}",
                    event.from
                ))
                .into());
            }
            let from_path = self.object_path_impl(&obj_def.name, &event.from, fs)?;
            let to_path = self.object_path_impl(&obj_def.name, &event.to, fs)?;
            let content = fs.read(&from_path)?.ok_or(ArchivalError::new(&format!(
                "file not found: {}",
                event.from
            )))?;
            fs.write(&to_path, content)?;
            fs.delete(&from_path)?;
            self.site.invalidate_file(&from_path);
            Ok(())
        })?;
        Ok(ArchivalEventResponse::None)
    }

    fn delete_object(
        &self,
        event: DeleteObjectEvent,
    ) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        let obj_def = self
            .site
            .object_definitions
            .get(&event.object)
            .ok_or(ArchivalError::new(&format!(
                "object not found: {}",
                event.object
            )))?;
        self.fs_mutex.with_fs(|fs| {
            let path = self.object_path_impl(&obj_def.name, &event.filename, fs)?;
            fs.delete(&path)?;
            self.site.invalidate_file(&path);
            Ok(())
        })?;
        Ok(ArchivalEventResponse::None)
    }

    pub fn manifest_content(&self) -> Result<String, Box<dyn Error>> {
        self.fs_mutex.with_fs(|fs| self.site.manifest_content(fs))
    }

    pub fn get_objects(&self) -> Result<ObjectMap, Box<dyn Error>> {
        self.fs_mutex.with_fs(|fs| self.site.get_objects(fs))
    }

    pub fn get_object(&self, name: &str, filename: Option<&str>) -> Result<Object, Box<dyn Error>> {
        self.fs_mutex
            .with_fs(|fs| self.site.get_object(name, filename, fs))
    }

    pub fn get_objects_sorted(
        &self,
        sort: impl Fn(&Object, &Object) -> Ordering,
    ) -> Result<ObjectMap, Box<dyn Error>> {
        self.fs_mutex
            .with_fs(|fs| self.site.get_objects_sorted(fs, Some(sort)))
    }

    fn edit_field(&self, event: EditFieldEvent) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        // TODO: we should probably validate all fields this way
        if let Some(FieldValue::Enum(enum_val)) = &event.value {
            let def = self
                .site
                .object_definitions
                .get(&event.object)
                .ok_or_else(|| {
                    ArchivalError::new(&format!("object type not found: {}", event.object))
                })?;
            let field = event
                .path
                .clone()
                .concat(ValuePath::from_string(&event.field))
                .get_field_definition(def)?;
            if let FieldType::Enum(valid_values) = field {
                if !valid_values.contains(enum_val) {
                    return Err(ArchivalError::new(&format!(
                        "Invalid value {enum_val} for enum [{}]",
                        valid_values.join(",")
                    ))
                    .into());
                }
            }
        }
        self.write_object(&event.object, &event.filename, |existing| {
            event
                .path
                .append((&event.field).into())
                .set_in_object(existing, event.value)?;
            Ok(existing)
        })?;
        Ok(ArchivalEventResponse::None)
    }
    fn edit_order(&self, event: EditOrderEvent) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        self.write_object(&event.object, &event.filename, |existing| {
            existing.order = event.order;
            Ok(existing)
        })?;
        Ok(ArchivalEventResponse::None)
    }

    fn add_child(&self, event: AddChildEvent) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        let mut added_idx = usize::MAX;
        self.write_object(&event.object, &event.filename, |existing| {
            added_idx = event.path.add_child(existing, event.index, |child| {
                for value in event.values {
                    value.path.set_in_tree(child, Some(value.value))?;
                }
                Ok(())
            })?;
            Ok(existing)
        })?;
        Ok(ArchivalEventResponse::Index(added_idx))
    }
    fn remove_child(
        &self,
        event: RemoveChildEvent,
    ) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        self.write_object(&event.object, &event.filename, move |existing| {
            let mut path = event.path;
            path.remove_child(existing)?;
            Ok(existing)
        })?;
        Ok(ArchivalEventResponse::None)
    }

    fn write_object(
        &self,
        obj_type: &str,
        filename: &str,
        obj_cb: impl FnOnce(&mut Object) -> Result<&mut Object, Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        debug!("write object {}: {}", obj_type, filename);
        self.fs_mutex.with_fs(|fs| {
            let path = self.object_path_impl(obj_type, filename, fs)?;
            let contents = self.modify_object_file(obj_type, filename, obj_cb, fs)?;
            fs.write_str(&path, contents)?;
            self.site.invalidate_file(&path);
            Ok(())
        })
    }

    pub fn modify_manifest(
        &mut self,
        modify: impl FnOnce(&mut Manifest),
    ) -> Result<(), Box<dyn Error>> {
        self.fs_mutex.with_fs(|fs| {
            self.site.modify_manifest(fs, modify)?;
            Ok(())
        })
    }

    /// Deletes all the object files for the given object types, except the ones
    /// (optionally) specified in the list of keep_objects.
    /// This only deletes the files, and does not generate events or rebuild the
    /// archival site.
    pub fn delete_objects(
        &self,
        object_names: impl IntoIterator<Item = impl AsRef<str>>,
        keep_objects: Option<Vec<ValuePath>>,
    ) -> Result<(), Box<dyn Error>> {
        let objects = self.get_objects()?;
        self.fs_mutex.with_fs(|fs| {
            for on in object_names {
                let object_name = on.as_ref();
                let current_path: ValuePath =
                    ValuePath::empty().append(ValuePath::key(object_name));
                if keep_objects
                    .as_ref()
                    .is_some_and(|ko| ko.contains(&current_path))
                {
                    continue;
                }
                let entry = objects.get(object_name).ok_or_else(|| {
                    ArchivalError::new(&format!("object {} does not exist", object_name))
                })?;
                let filenames = match entry {
                    ObjectEntry::Object(object) => {
                        vec![&object.filename]
                    }
                    ObjectEntry::List(objects) => objects.iter().map(|o| &o.filename).collect(),
                };
                for filename in filenames {
                    let current_path = current_path.clone().append(ValuePath::key(filename));
                    if keep_objects
                        .as_ref()
                        .is_some_and(|ko| ko.contains(&current_path))
                    {
                        continue;
                    }
                    let path = self.object_path_impl(object_name, filename, fs)?;
                    fs.delete(&path)?;
                    self.site.invalidate_file(&path);
                }
            }
            Ok(())
        })
    }

    pub fn take_fs(self) -> F {
        self.fs_mutex.take_fs()
    }
    pub fn clone_fs(&self) -> Result<F, Box<dyn Error>> {
        self.fs_mutex.with_fs(|fs| Ok(fs.clone()))
    }
}

#[cfg(test)]
mod lib {
    use std::error::Error;

    use crate::{file_system::unpack_zip, test_utils::as_path_str, value_path::ValuePath};
    use events::AddObjectValue;
    use tracing_test::traced_test;

    use super::*;

    #[test]
    #[traced_test]
    fn load_and_build_site_from_zip() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        assert_eq!(archival.site.object_definitions.len(), 4);
        assert!(archival.site.object_definitions.contains_key("section"));
        assert!(archival.site.object_definitions.contains_key("post"));
        assert!(archival.site.object_definitions.contains_key("site"));
        let objects = archival.get_objects()?;
        let section_objs = objects.get("section").unwrap();
        assert!(matches!(section_objs, ObjectEntry::List(_)));
        let site_obj = objects.get("site").unwrap();
        assert!(matches!(site_obj, ObjectEntry::Object(_)));
        let post_obj = objects.get("post").unwrap();
        assert!(matches!(post_obj, ObjectEntry::List(_)));
        let fp = post_obj.into_iter().next().unwrap();
        let m = ValuePath::from_string("media.0.image")
            .get_in_object(fp)
            .unwrap();
        assert!(matches!(m, FieldValue::File(_)));
        if let FieldValue::File(img) = m {
            assert_eq!(img.filename, "test.jpg");
            assert_eq!(img.mime, "image/jpg");
            assert_eq!(img.name, Some("Test".to_string()));
            assert_eq!(img.sha, "test-sha");
            assert_eq!(img.url, "test://uploads-url/test-sha/test.jpg");
        }
        archival.build(BuildOptions::default())?;
        let dist_files = archival
            .dist_files()
            .into_iter()
            .map(|f| f.path.display().to_string())
            .collect::<Vec<String>>();
        println!("dist_files: \n{:#?}", dist_files);
        assert!(dist_files.contains(&as_path_str("index.html")));
        assert!(dist_files.contains(&as_path_str("404.html")));
        assert!(dist_files.contains(&as_path_str("post/a-post.html")));
        assert!(dist_files.contains(&as_path_str("img/guy.webp")));
        assert!(dist_files.contains(&as_path_str("rss.rss")));
        assert_eq!(dist_files.len(), 17);
        let guy = archival.dist_file(Path::new("img/guy.webp"));
        assert!(guy.is_some());
        let post_html = archival
            .fs_mutex
            .with_fs(|fs| {
                fs.read_to_string(archival.site.manifest.build_dir.join("post/a-post.html"))
            })?
            .unwrap();
        println!("{}", post_html);
        assert!(post_html.contains("test://uploads-url/test-sha/test.jpg"));
        assert!(post_html.contains("title=\"Test\""));
        Ok(())
    }

    #[test]
    fn add_object_to_site() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival.send_event(
            ArchivalEvent::AddObject(AddObjectEvent {
                object: "section".to_string(),
                filename: "my-section".to_string(),
                order: Some(3.),
                // Sections require a name field, so we have to add it or we'll get a build error
                values: vec![AddObjectValue {
                    path: ValuePath::from_string("name"),
                    value: FieldValue::String("section three".to_string()),
                }],
            }),
            Some(BuildOptions::default()),
        )?;
        // Sending an event should result in an updated fs
        let sections_dir = archival.site.manifest.objects_dir.join("section");
        let sections = archival.fs_mutex.with_fs(|fs| {
            fs.walk_dir(&sections_dir, false)
                .map(|d| d.collect::<Vec<PathBuf>>())
        })?;
        println!("SECTIONS: {:?}", sections);
        assert_eq!(sections.len(), 3);
        let section_toml = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(sections_dir.join("my-section.toml")));
        assert!(section_toml.is_ok());
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(archival.site.manifest.build_dir.join("index.html")))?
            .unwrap();
        let rendered_sections: Vec<_> = index_html.match_indices("<h2>").collect();
        println!("MATCHED: {:?}", rendered_sections);
        assert_eq!(rendered_sections.len(), 3);
        Ok(())
    }

    #[test]
    fn edit_object() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival.send_event(
            ArchivalEvent::EditField(EditFieldEvent {
                object: "section".to_string(),
                filename: "first".to_string(),
                path: ValuePath::empty(),
                field: "name".to_string(),
                value: Some(FieldValue::String("This is the new name".to_string())),
                source: None,
            }),
            Some(BuildOptions::default()),
        )?;
        // Sending an event should result in an updated fs
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(archival.site.manifest.build_dir.join("index.html")))?
            .unwrap();
        println!("index: {}", index_html);
        assert!(index_html.contains("This is the new name"));
        Ok(())
    }

    #[test]
    fn delete_object() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival.send_event(
            ArchivalEvent::DeleteObject(DeleteObjectEvent {
                object: "section".to_string(),
                filename: "first".to_string(),
                source: None,
            }),
            Some(BuildOptions::default()),
        )?;
        // Sending an event should result in an updated fs
        let sections_dir = archival.site.manifest.objects_dir.join("section");
        let sections = archival.fs_mutex.with_fs(|fs| {
            fs.walk_dir(&sections_dir, false)
                .map(|d| d.collect::<Vec<PathBuf>>())
        })?;
        println!("SECTIONS: {:?}", sections);
        assert_eq!(sections.len(), 1);
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(archival.site.manifest.build_dir.join("index.html")))?
            .unwrap();
        println!("index: {}", index_html);
        assert!(!index_html.contains("This is the new title"));
        Ok(())
    }

    #[test]
    fn rename_object() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        let sections_dir = archival.site.manifest.objects_dir.join("section");
        let sections_before_rename = archival.fs_mutex.with_fs(|fs| {
            fs.walk_dir(&sections_dir, false)
                .map(|d| d.collect::<Vec<PathBuf>>())
        })?;
        archival.send_event(
            ArchivalEvent::RenameObject(RenameObjectEvent {
                object: "section".to_string(),
                from: "first".to_string(),
                to: "renamed".to_string(),
            }),
            Some(BuildOptions::default()),
        )?;
        // Sending an event should result in an updated fs
        let sections_dir = archival.site.manifest.objects_dir.join("section");
        let sections = archival.fs_mutex.with_fs(|fs| {
            fs.walk_dir(&sections_dir, false)
                .map(|d| d.collect::<Vec<PathBuf>>())
        })?;
        println!("SECTIONS: {:?}", sections);
        assert_eq!(sections.len(), sections_before_rename.len());
        assert!(sections.iter().any(|path| path.ends_with("renamed.toml")));
        Ok(())
    }

    #[test]
    #[traced_test]
    fn edit_object_order() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival.build(BuildOptions::default())?;
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(archival.site.manifest.build_dir.join("index.html")))?
            .unwrap();
        println!("index: {}", index_html);
        let c1 = index_html.find("1 Some Content").unwrap();
        let c2 = index_html.find("2 More Content").unwrap();
        assert!(c1 < c2);
        archival.send_event(
            ArchivalEvent::EditOrder(EditOrderEvent {
                object: "section".to_string(),
                filename: "first".to_string(),
                order: Some(12.),
                source: None,
            }),
            Some(BuildOptions::default()),
        )?;
        // Sending an event should result in an updated fs
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(archival.site.manifest.build_dir.join("index.html")))?
            .unwrap();
        println!("index: {}", index_html);
        let c1 = index_html.find("12 Some Content").unwrap();
        let c2 = index_html.find("2 More Content").unwrap();
        assert!(c2 < c1);
        Ok(())
    }

    #[test]
    #[traced_test]
    fn add_child() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival.build(BuildOptions::default())?;
        let post_html = archival
            .fs_mutex
            .with_fs(|fs| {
                fs.read_to_string(
                    archival
                        .site
                        .manifest
                        .build_dir
                        .join(Path::new("post/a-post.html")),
                )
            })?
            .unwrap();
        // println!("post: {}", post_html);
        let rendered_links: Vec<_> = post_html.match_indices("<a href=").collect();
        // println!("LINKS: {:?}", rendered_links);
        assert_eq!(rendered_links.len(), 2);
        archival
            .send_event(
                ArchivalEvent::AddChild(AddChildEvent {
                    object: "post".to_string(),
                    filename: "a-post".to_string(),
                    path: ValuePath::default().append(ValuePath::key("links")),
                    values: vec![
                        AddObjectValue {
                            path: ValuePath::from_string("url"),
                            value: FieldValue::String("http://foo.com".to_string()),
                        },
                        AddObjectValue {
                            path: ValuePath::from_string("name"),
                            value: FieldValue::String("another link".to_string()),
                        },
                    ],
                    index: None,
                }),
                Some(BuildOptions::no_static()),
            )
            .unwrap();
        let objects = archival.get_objects()?;
        let posts = objects.get("post").unwrap();
        let mut found = false;
        for post in posts {
            if post.filename == "a-post" {
                found = true;
                let links = ValuePath::from_string("links").get_in_object(post).unwrap();
                assert!(matches!(links, FieldValue::Objects(_)));
                if let FieldValue::Objects(links) = links {
                    assert_eq!(links.len(), 3);
                }
            }
        }
        assert!(found);
        // Sending an event should result in an updated fs
        let post_html = archival
            .fs_mutex
            .with_fs(|fs| {
                fs.read_to_string(
                    archival
                        .site
                        .manifest
                        .build_dir
                        .join(Path::new("post/a-post.html")),
                )
            })?
            .unwrap();
        println!("post: {}", post_html);
        let rendered_links: Vec<_> = post_html.match_indices("<a href=").collect();
        println!("LINKS: {:?}", rendered_links);
        assert_eq!(rendered_links.len(), 3);
        let rendered_link_url: Vec<_> = post_html.match_indices("foo.com").collect();
        assert_eq!(rendered_link_url.len(), 1);
        let rendered_link_name: Vec<_> = post_html.match_indices("another link").collect();
        assert_eq!(rendered_link_name.len(), 1);
        Ok(())
    }

    #[test]
    fn remove_child() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival
            .send_event(
                ArchivalEvent::RemoveChild(RemoveChildEvent {
                    object: "post".to_string(),
                    filename: "a-post".to_string(),
                    path: ValuePath::default()
                        .append(ValuePath::key("links"))
                        .append(ValuePath::index(0)),
                    source: None,
                }),
                Some(BuildOptions::default()),
            )
            .unwrap();
        // Sending an event should result in an updated fs
        let post_html = archival
            .fs_mutex
            .with_fs(|fs| {
                fs.read_to_string(
                    archival
                        .site
                        .manifest
                        .build_dir
                        .join(Path::new("post/a-post.html")),
                )
            })?
            .unwrap();
        println!("post: {}", post_html);
        let rendered_links: Vec<_> = post_html.match_indices("<a href=").collect();
        println!("LINKS: {:?}", rendered_links);
        assert_eq!(rendered_links.len(), 1);
        Ok(())
    }

    #[test]
    fn modify_manifest() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let mut archival = Archival::new(fs)?;
        archival.modify_manifest(|m| {
            m.site_url = Some("test.com".to_string());
            m.prebuild = vec!["test".to_string()];
        })?;
        let output = archival.site.manifest.to_toml()?;
        println!("{}", output);
        assert!(output.contains("site_url = \"test.com\""));
        // Doesn't fill defaults
        assert!(!output.contains("objects_dir"));
        assert!(!output.contains("objects"));
        // Does show non-defaults
        assert!(output.contains("prebuild = [\"test\"]"));
        Ok(())
    }

    #[test]
    fn bulk_delete_objects() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival.delete_objects(
            vec!["section", "site"].into_iter(),
            Some(vec![ValuePath::from_string("section.second")]),
        )?;
        // This should result in the relevant files being missing
        let sections_dir = archival.site.manifest.objects_dir.join("section");
        let sections = archival.fs_mutex.with_fs(|fs| {
            fs.walk_dir(&sections_dir, false)
                .map(|d| d.collect::<Vec<PathBuf>>())
        })?;
        println!("SECTIONS: {:?}", sections);
        assert_eq!(sections.len(), 1);
        let site_file_exists = archival
            .fs_mutex
            .with_fs(|fs| fs.exists(archival.site.manifest.objects_dir.join("site.toml")))?;
        assert!(!site_file_exists);
        Ok(())
    }

    #[test]
    #[traced_test]
    fn build_ids() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        let initial_fs_id = archival.fs_id()?;
        debug!("INITIAL FS ID: {:?}", initial_fs_id);
        archival.build(BuildOptions::default())?;
        let initial_build_id = archival.build_id();
        debug!("INITIAL BUILD ID: {:?}", initial_build_id);
        assert_eq!(
            archival.fs_id()?,
            initial_fs_id,
            "fs id changed but there was no change"
        );
        archival.send_event(
            ArchivalEvent::EditField(EditFieldEvent {
                object: "section".to_string(),
                filename: "first".to_string(),
                path: ValuePath::empty(),
                field: "name".to_string(),
                value: Some(FieldValue::String("This is the new name".to_string())),
                source: None,
            }),
            None,
        )?;
        assert_ne!(
            archival.fs_id()?,
            initial_fs_id,
            "fs id did not change after file changes"
        );
        assert_eq!(
            archival.build_id(),
            initial_build_id,
            "build id changed without a dist change"
        );
        archival.build(BuildOptions::default())?;
        assert_ne!(
            archival.build_id(),
            initial_build_id,
            "build id did not change after build"
        );
        Ok(())
    }
    #[test]
    fn edit_enum() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival.send_event(
            ArchivalEvent::EditField(EditFieldEvent {
                object: "post".to_string(),
                filename: "a-post".to_string(),
                path: ValuePath::empty(),
                field: "state".to_string(),
                value: Some(FieldValue::Enum("draft".to_string())),
                source: None,
            }),
            Some(BuildOptions::default()),
        )?;
        let post_html = archival
            .fs_mutex
            .with_fs(|fs| {
                fs.read_to_string(
                    archival
                        .site
                        .manifest
                        .build_dir
                        .join(Path::new("post/a-post.html")),
                )
            })?
            .unwrap();
        println!("post: {}", post_html);
        assert!(post_html.contains("State: draft"));
        Ok(())
    }
    #[test]
    fn edit_enum_fails_when_invalid() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        assert!(archival
            .send_event(
                ArchivalEvent::EditField(EditFieldEvent {
                    object: "post".to_string(),
                    filename: "a-post".to_string(),
                    path: ValuePath::empty(),
                    field: "state".to_string(),
                    value: Some(FieldValue::Enum("poopoo".to_string())),
                    source: None,
                }),
                Some(BuildOptions::default()),
            )
            .is_err());
        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "typescript")]
mod typescript_definitions {
    use typescript_type_def::{write_definition_file, DefinitionFileOptions};
    use value_path::ValuePath;

    use crate::fields::FieldType;

    use super::*;

    #[test]
    fn run() {
        let mut buf = Vec::new();
        let options = DefinitionFileOptions {
            header: Some("// AUTO-GENERATED by typescript-type-def\n"),
            root_namespace: None,
        };
        type ExportedTypes = (
            ArchivalEvent,
            ObjectDefinition,
            Object,
            ValuePath,
            FieldType,
            ObjectEntry,
        );
        write_definition_file::<_, ExportedTypes>(&mut buf, options).unwrap();
    }
}

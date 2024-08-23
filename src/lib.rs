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
mod value_path;
use constants::MIN_COMPAT_VERSION;
use events::ArchivalEventResponse;
use events::{
    AddObjectEvent, ArchivalEvent, ChildEvent, DeleteObjectEvent, EditFieldEvent, EditOrderEvent,
};
pub mod fields;
pub use fields::FieldConfig;
pub use fields::FieldValue;
use manifest::Manifest;
use sha2::{Digest, Sha256};
use site::Site;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use tracing::{debug, error};
#[cfg(feature = "binary")]
pub mod binary;
mod constants;
#[cfg(feature = "stdlib-fs")]
mod file_system_stdlib;
#[cfg(feature = "binary")]
mod server;
use file_system_mutex::FileSystemMutex;
use object::{Object, ObjectEntry};
use semver::{Version, VersionReq};

// Re-exports
pub mod events;
pub mod object;
pub use archival_error::ArchivalError;
pub use file_system::unpack_zip;
pub use file_system::FileSystemAPI;
pub use file_system_memory::MemoryFileSystem;
pub use object_definition::ObjectDefinition;

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

pub struct Archival<F: FileSystemAPI> {
    fs_mutex: FileSystemMutex<F>,
    pub site: site::Site,
}

impl<F: FileSystemAPI> Archival<F> {
    pub fn is_compatible(fs: &F) -> Result<bool, Box<dyn Error>> {
        let site = Site::load(fs)?;
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
        let site = Site::load(&fs)?;
        FieldConfig::set(site.get_field_config());
        let fs_mutex = FileSystemMutex::init(fs);
        Ok(Self { fs_mutex, site })
    }
    pub fn new_with_field_config(fs: F, field_config: FieldConfig) -> Result<Self, Box<dyn Error>> {
        let site = Site::load(&fs)?;
        FieldConfig::set(field_config);
        let fs_mutex = FileSystemMutex::init(fs);
        Ok(Self { fs_mutex, site })
    }
    pub fn build(&self) -> Result<(), Box<dyn Error>> {
        debug!("build {}", self.site);
        self.fs_mutex.with_fs(|fs| self.site.build(fs))
    }
    pub fn dist_file(&self, path: &Path) -> Option<Vec<u8>> {
        let path = self.site.manifest.build_dir.join(path);
        self.fs_mutex.with_fs(|fs| fs.read(&path)).unwrap_or(None)
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
    fn send_event_impl(
        &self,
        event: ArchivalEvent,
        rebuild: bool,
    ) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        let r = match event {
            ArchivalEvent::AddObject(event) => self.add_object(event)?,
            ArchivalEvent::DeleteObject(event) => self.delete_object(event)?,
            ArchivalEvent::EditField(event) => self.edit_field(event)?,
            ArchivalEvent::EditOrder(event) => self.edit_order(event)?,
            ArchivalEvent::AddChild(event) => self.add_child(event)?,
            ArchivalEvent::RemoveChild(event) => self.remove_child(event)?,
        };
        if rebuild {
            self.build()?;
        }
        Ok(r)
    }
    pub fn send_event_no_rebuild(
        &self,
        event: ArchivalEvent,
    ) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        self.send_event_impl(event, false)
    }
    pub fn send_event(
        &self,
        event: ArchivalEvent,
    ) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        self.send_event_impl(event, true)
    }
    // Internal
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
            let path = self.object_path_impl(&obj_def.name, &event.filename, fs)?;
            let object = Object::from_def(obj_def, &event.filename, event.order, event.values)?;
            fs.write_str(&path, object.to_toml()?)?;
            self.site.invalidate_file(&path);
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

    pub fn get_objects(&self) -> Result<HashMap<String, ObjectEntry>, Box<dyn Error>> {
        self.fs_mutex.with_fs(|fs| self.site.get_objects(fs))
    }

    pub fn get_objects_sorted(
        &self,
        sort: impl Fn(&Object, &Object) -> Ordering,
    ) -> Result<HashMap<String, ObjectEntry>, Box<dyn Error>> {
        self.fs_mutex
            .with_fs(|fs| self.site.get_objects_sorted(fs, Some(sort)))
    }

    fn edit_field(&self, event: EditFieldEvent) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        self.write_object(&event.object, &event.filename, |existing| {
            event
                .path
                .append((&event.field).into())
                .set_in_object(existing, event.value);
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

    fn add_child(&self, event: ChildEvent) -> Result<ArchivalEventResponse, Box<dyn Error>> {
        let obj_def = self
            .site
            .object_definitions
            .get(&event.object)
            .ok_or(ArchivalError::new(&format!(
                "object not found: {}",
                event.object
            )))?;
        let mut added_idx = usize::MAX;
        self.write_object(&event.object, &event.filename, |existing| {
            added_idx = event.path.add_child(existing, obj_def)?;
            Ok(existing)
        })?;
        Ok(ArchivalEventResponse::Index(added_idx))
    }
    fn remove_child(&self, event: ChildEvent) -> Result<ArchivalEventResponse, Box<dyn Error>> {
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
        debug!("write {}", obj_type);
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

    pub fn take_fs(self) -> F {
        self.fs_mutex.take_fs()
    }

    #[cfg(test)]
    pub fn dist_files(&self) -> Vec<String> {
        let mut files = vec![];
        self.fs_mutex
            .with_fs(|fs| {
                for file in fs.walk_dir(&self.site.manifest.build_dir, true)? {
                    files.push(file.display().to_string());
                }
                Ok(())
            })
            .unwrap();
        files
    }
}

#[cfg(test)]
mod lib {
    use std::error::Error;

    use crate::{
        file_system::unpack_zip,
        test_utils::as_path_str,
        value_path::{ValuePath, ValuePathComponent},
    };
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
            assert_eq!(img.url, "test://uploads-url/test-sha");
        }
        archival.build()?;
        let dist_files = archival.dist_files();
        println!("dist_files: \n{}", dist_files.join("\n"));
        assert!(archival.dist_files().contains(&as_path_str("index.html")));
        assert!(archival.dist_files().contains(&as_path_str("404.html")));
        assert!(archival
            .dist_files()
            .contains(&as_path_str("post/a-post.html")));
        assert!(archival.dist_files().contains(&as_path_str("img/guy.webp")));
        assert!(archival.dist_files().contains(&as_path_str("rss.rss")));
        assert_eq!(dist_files.len(), 20);
        let guy = archival.dist_file(Path::new("img/guy.webp"));
        assert!(guy.is_some());
        let post_html = archival
            .fs_mutex
            .with_fs(|fs| {
                fs.read_to_string(&archival.site.manifest.build_dir.join("post/a-post.html"))
            })?
            .unwrap();
        println!("{}", post_html);
        assert!(post_html.contains("test://uploads-url/test-sha"));
        assert!(post_html.contains("title=\"Test\""));
        Ok(())
    }

    #[test]
    fn add_object_to_site() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival.send_event(ArchivalEvent::AddObject(AddObjectEvent {
            object: "section".to_string(),
            filename: "my-section".to_string(),
            order: 3,
            // Sections require a name field, so we have to add it or we'll get a build error
            values: vec![AddObjectValue {
                path: ValuePath::from_string("name"),
                value: FieldValue::String("section three".to_string()),
            }],
        }))?;
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
            .with_fs(|fs| fs.read_to_string(&sections_dir.join("my-section.toml")));
        assert!(section_toml.is_ok());
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(&archival.site.manifest.build_dir.join("index.html")))?
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
        archival.send_event(ArchivalEvent::EditField(EditFieldEvent {
            object: "section".to_string(),
            filename: "first".to_string(),
            path: ValuePath::empty(),
            field: "name".to_string(),
            value: Some(FieldValue::String("This is the new name".to_string())),
        }))?;
        // Sending an event should result in an updated fs
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(&archival.site.manifest.build_dir.join("index.html")))?
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
        archival.send_event(ArchivalEvent::DeleteObject(DeleteObjectEvent {
            object: "section".to_string(),
            filename: "first".to_string(),
        }))?;
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
            .with_fs(|fs| fs.read_to_string(&archival.site.manifest.build_dir.join("index.html")))?
            .unwrap();
        println!("index: {}", index_html);
        assert!(!index_html.contains("This is the new title"));
        Ok(())
    }

    #[test]
    #[traced_test]
    fn edit_object_order() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival.build()?;
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(&archival.site.manifest.build_dir.join("index.html")))?
            .unwrap();
        println!("index: {}", index_html);
        let c1 = index_html.find("1 Some Content").unwrap();
        let c2 = index_html.find("2 More Content").unwrap();
        assert!(c1 < c2);
        archival.send_event(ArchivalEvent::EditOrder(EditOrderEvent {
            object: "section".to_string(),
            filename: "first".to_string(),
            order: 12,
        }))?;
        // Sending an event should result in an updated fs
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(&archival.site.manifest.build_dir.join("index.html")))?
            .unwrap();
        println!("index: {}", index_html);
        let c1 = index_html.find("12 Some Content").unwrap();
        let c2 = index_html.find("2 More Content").unwrap();
        assert!(c2 < c1);
        Ok(())
    }

    #[test]
    fn add_child() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival.build()?;
        let post_html = archival
            .fs_mutex
            .with_fs(|fs| {
                fs.read_to_string(
                    &archival
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
        assert_eq!(rendered_links.len(), 1);
        archival
            .send_event(ArchivalEvent::AddChild(ChildEvent {
                object: "post".to_string(),
                filename: "a-post".to_string(),
                path: ValuePath::default().append(ValuePathComponent::key("links")),
            }))
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
                    assert_eq!(links.len(), 2);
                }
            }
        }
        assert!(found);
        // Sending an event should result in an updated fs
        let post_html = archival
            .fs_mutex
            .with_fs(|fs| {
                fs.read_to_string(
                    &archival
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
        assert_eq!(rendered_links.len(), 2);
        Ok(())
    }

    #[test]
    fn remove_child() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        archival
            .send_event(ArchivalEvent::RemoveChild(ChildEvent {
                object: "post".to_string(),
                filename: "a-post".to_string(),
                path: ValuePath::default()
                    .append(ValuePathComponent::key("links"))
                    .append(ValuePathComponent::Index(0)),
            }))
            .unwrap();
        // Sending an event should result in an updated fs
        let post_html = archival
            .fs_mutex
            .with_fs(|fs| {
                fs.read_to_string(
                    &archival
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
        assert_eq!(rendered_links.len(), 0);
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
            m.archival_site = Some("test".to_string());
            m.prebuild = vec!["test".to_string()];
        })?;
        let output = archival.site.manifest.to_toml()?;
        println!("{}", output);
        assert!(output.contains("archival_site = \"test\""));
        assert!(output.contains("site_url = \"test.com\""));
        // Doesn't fill defaults
        assert!(!output.contains("objects_dir"));
        assert!(!output.contains("objects"));
        // Does show non-defaults
        assert!(output.contains("prebuild = [\"test\"]"));
        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "typescript")]
mod typescript_tests {
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

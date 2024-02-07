mod archival_error;
mod field_value;
mod file_system;
mod file_system_memory;
mod file_system_mutex;
#[cfg(test)]
mod file_system_tests;
mod filters;
mod liquid_parser;
mod manifest;
mod object_definition;
mod page;
mod read_toml;
mod reserved_fields;
mod site;
mod tags;
mod value_path;
use events::{
    AddObjectEvent, ArchivalEvent, ChildEvent, DeleteObjectEvent, EditFieldEvent, EditOrderEvent,
};
pub use field_value::FieldValue;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use tracing::debug;
#[cfg(feature = "binary")]
pub mod binary;
mod constants;
#[cfg(feature = "stdlib-fs")]
mod file_system_stdlib;
#[cfg(feature = "wasm-fs")]
mod file_system_wasm;
use file_system_mutex::FileSystemMutex;
use object::Object;

// Re-exports
pub mod events;
pub mod object;
pub use archival_error::ArchivalError;
pub use file_system::unpack_zip;
pub use file_system::FileSystemAPI;
pub use file_system_memory::MemoryFileSystem;
#[cfg(feature = "wasm-fs")]
pub use file_system_wasm::WasmFileSystem;
pub use object_definition::ObjectDefinition;

pub struct Archival<F: FileSystemAPI> {
    fs_mutex: FileSystemMutex<F>,
    pub site: site::Site,
}

impl<F: FileSystemAPI> Archival<F> {
    pub fn new(fs: F) -> Result<Self, Box<dyn Error>> {
        let site = site::load(&fs)?;
        let fs_mutex = FileSystemMutex::init(fs);
        Ok(Self { fs_mutex, site })
    }
    pub fn build(&self) -> Result<(), Box<dyn Error>> {
        debug!("build {}", self.site);
        self.fs_mutex.with_fs(|fs| site::build(&self.site, fs))
    }
    pub fn dist_file(&self, path: &Path) -> Option<Vec<u8>> {
        let path = self.site.manifest.build_dir.join(path);
        self.fs_mutex.with_fs(|fs| fs.read(&path)).unwrap_or(None)
    }
    pub fn object_path(&self, obj_type: &str, filename: &str) -> PathBuf {
        self.site
            .manifest
            .objects_dir
            .join(Path::new(&obj_type))
            .join(Path::new(&format!("{}.toml", filename)))
    }
    pub fn object_file(&self, obj_type: &str, filename: &str) -> Result<String, Box<dyn Error>> {
        self.object_file_with(obj_type, filename, |o| Ok(o))
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
            .objects
            .get(obj_type)
            .ok_or(ArchivalError::new(&format!(
                "object not found: {}",
                obj_type
            )))?;
        let table: toml::Table = toml::from_str(&contents)?;
        let _ = Object::from_table(obj_def, filename, &table)?;
        // Object is valid, write it
        self.fs_mutex
            .with_fs(|fs| fs.write_str(&self.object_path(obj_type, filename), contents))
    }
    fn object_file_with(
        &self,
        obj_type: &str,
        filename: &str,
        obj_cb: impl FnOnce(&mut Object) -> Result<&mut Object, Box<dyn Error>>,
    ) -> Result<String, Box<dyn Error>> {
        let mut all_objects = self.get_objects()?;
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
    pub fn send_event(&self, event: ArchivalEvent) -> Result<(), Box<dyn Error>> {
        match event {
            ArchivalEvent::AddObject(event) => self.add_object(event)?,
            ArchivalEvent::DeleteObject(event) => self.delete_object(event)?,
            ArchivalEvent::EditField(event) => self.edit_field(event)?,
            ArchivalEvent::EditOrder(event) => self.edit_order(event)?,
            ArchivalEvent::AddChild(event) => self.add_child(event)?,
            ArchivalEvent::RemoveChild(event) => self.remove_child(event)?,
        }
        // After any event, rebuild
        self.build()
    }
    // Internal
    fn add_object(&self, event: AddObjectEvent) -> Result<(), Box<dyn Error>> {
        let obj_def = self
            .site
            .objects
            .get(&event.object)
            .ok_or(ArchivalError::new(&format!(
                "object not found: {}",
                event.object
            )))?;
        let path = self
            .site
            .manifest
            .objects_dir
            .join(Path::new(&obj_def.name))
            .join(Path::new(&format!("{}.toml", event.filename)));
        self.fs_mutex.with_fs(|fs| {
            let object = Object::from_def(obj_def, &event.filename, event.order)?;
            fs.write_str(&path, object.to_toml()?)
        })
    }

    fn delete_object(&self, event: DeleteObjectEvent) -> Result<(), Box<dyn Error>> {
        let obj_def = self
            .site
            .objects
            .get(&event.object)
            .ok_or(ArchivalError::new(&format!(
                "object not found: {}",
                event.object
            )))?;
        let path = self
            .site
            .manifest
            .objects_dir
            .join(Path::new(&obj_def.name))
            .join(Path::new(&format!("{}.toml", event.filename)));
        self.fs_mutex.with_fs(|fs| fs.delete(&path))
    }

    pub fn get_objects(&self) -> Result<HashMap<String, Vec<Object>>, Box<dyn Error>> {
        self.fs_mutex
            .with_fs(|fs| site::get_objects(&self.site, fs))
    }
    pub fn get_objects_sorted(
        &self,
        sort: impl Fn(&Object, &Object) -> Ordering,
    ) -> Result<HashMap<String, Vec<Object>>, Box<dyn Error>> {
        self.fs_mutex
            .with_fs(|fs| site::get_objects_sorted(&self.site, fs, sort))
    }

    fn edit_field(&self, event: EditFieldEvent) -> Result<(), Box<dyn Error>> {
        self.write_object(&event.object, &event.filename, |existing| {
            event.path.set_in_object(existing, event.value);
            Ok(existing)
        })?;
        Ok(())
    }
    fn edit_order(&self, event: EditOrderEvent) -> Result<(), Box<dyn Error>> {
        self.write_object(&event.object, &event.filename, |existing| {
            existing.order = event.order;
            Ok(existing)
        })?;
        Ok(())
    }

    fn add_child(&self, event: ChildEvent) -> Result<(), Box<dyn Error>> {
        let obj_def = self
            .site
            .objects
            .get(&event.object)
            .ok_or(ArchivalError::new(&format!(
                "object not found: {}",
                event.object
            )))?;
        self.write_object(&event.object, &event.filename, move |existing| {
            event.path.add_child(existing, obj_def)?;
            Ok(existing)
        })?;
        Ok(())
    }
    fn remove_child(&self, event: ChildEvent) -> Result<(), Box<dyn Error>> {
        self.write_object(&event.object, &event.filename, move |existing| {
            let mut path = event.path;
            path.remove_child(existing)?;
            Ok(existing)
        })?;
        Ok(())
    }

    fn write_object(
        &self,
        obj_type: &str,
        filename: &str,
        obj_cb: impl FnOnce(&mut Object) -> Result<&mut Object, Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        debug!("WRITE {}", filename);
        let path = self.object_path(obj_type, filename);
        let contents = self.object_file_with(obj_type, filename, obj_cb)?;
        self.fs_mutex.with_fs(|fs| fs.write_str(&path, contents))
    }

    #[cfg(test)]
    pub fn dist_files(&self) -> Vec<String> {
        let mut files = vec![];
        self.fs_mutex
            .with_fs(|fs| {
                for file in fs.walk_dir(&self.site.manifest.build_dir)? {
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
        value_path::{ValuePath, ValuePathComponent},
    };
    use tracing_test::traced_test;

    use super::*;

    #[test]
    #[traced_test]
    fn load_and_build_site_from_zip() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        assert_eq!(archival.site.objects.len(), 2);
        assert!(archival.site.objects.contains_key("section"));
        assert!(archival.site.objects.contains_key("post"));
        archival.build()?;
        let dist_files = archival.dist_files();
        assert!(archival
            .dist_files()
            .contains(&"dist/index.html".to_owned()));
        assert!(archival.dist_files().contains(&"dist/404.html".to_owned()));
        assert!(archival
            .dist_files()
            .contains(&"dist/post/a-post.html".to_owned()));
        assert!(archival
            .dist_files()
            .contains(&"dist/img/guy.webp".to_owned()));
        assert_eq!(dist_files.len(), 18);
        let guy = archival.dist_file(Path::new("img/guy.webp"));
        assert!(guy.is_some());
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
        }))?;
        // Sending an event should result in an updated fs
        let sections_dir = archival.site.manifest.objects_dir.join("section");
        let sections = archival.fs_mutex.with_fs(|fs| fs.read_dir(&sections_dir))?;
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
            path: ValuePath::default().join(value_path::ValuePathComponent::key("name")),
            value: FieldValue::String("This is the new title".to_string()),
        }))?;
        // Sending an event should result in an updated fs
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(&archival.site.manifest.build_dir.join("index.html")))?
            .unwrap();
        println!("index: {}", index_html);
        assert!(index_html.contains("This is the new title"));
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
        let sections = archival.fs_mutex.with_fs(|fs| fs.read_dir(&sections_dir))?;
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
        assert!(index_html.contains("ORDER: Some Content,More Content"));
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
        assert!(index_html.contains("ORDER: More Content,Some Content"));
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
                fs.read_to_string(&archival.site.manifest.build_dir.join("post/a-post.html"))
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
                path: ValuePath::default().join(ValuePathComponent::key("links")),
            }))
            .unwrap();
        // Sending an event should result in an updated fs
        let post_html = archival
            .fs_mutex
            .with_fs(|fs| {
                fs.read_to_string(&archival.site.manifest.build_dir.join("post/a-post.html"))
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
        archival.build()?;
        archival
            .send_event(ArchivalEvent::RemoveChild(ChildEvent {
                object: "post".to_string(),
                filename: "a-post".to_string(),
                path: ValuePath::default()
                    .join(ValuePathComponent::key("links"))
                    .join(ValuePathComponent::Index(0)),
            }))
            .unwrap();
        // Sending an event should result in an updated fs
        let post_html = archival
            .fs_mutex
            .with_fs(|fs| {
                fs.read_to_string(&archival.site.manifest.build_dir.join("post/a-post.html"))
            })?
            .unwrap();
        println!("post: {}", post_html);
        let rendered_links: Vec<_> = post_html.match_indices("<a href=").collect();
        println!("LINKS: {:?}", rendered_links);
        assert_eq!(rendered_links.len(), 0);
        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "wasm-fs")]
mod wasm_tests {
    use std::error::Error;

    use crate::{unpack_zip, Archival, MemoryFileSystem};

    #[test]
    fn serialize_objects() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs)?;
        let _def_json = serde_json::to_string(&archival.site.objects)?;
        let _obj_json = serde_json::to_string(&archival.get_objects()?)?;
        Ok(())
    }
}

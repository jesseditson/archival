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
mod object;
mod object_definition;
mod page;
mod read_toml;
mod reserved_fields;
mod site;
mod tags;
use std::error::Error;
use std::path::Path;
// use std::sync::mpsc::{self, Receiver, Sender};

pub use archival_error::ArchivalError;
use events::{AddObjectEvent, ArchivalEvent, EditFieldEvent, EditOrderEvent};
pub use file_system::{FileSystemAPI, WatchableFileSystemAPI};
#[cfg(feature = "binary")]
pub mod binary;
mod constants;
#[cfg(feature = "stdlib-fs")]
mod file_system_stdlib;
#[cfg(feature = "wasm-fs")]
mod file_system_wasm;
pub use file_system::unpack_zip;
pub use file_system_memory::MemoryFileSystem;
use file_system_mutex::FileSystemMutex;
#[cfg(feature = "wasm-fs")]
pub use file_system_wasm::WasmFileSystem;
use object::Object;
use site::Site;
pub mod events;

#[cfg(feature = "wasm-fs")]
pub fn fetch_site(url: &str) -> Result<Vec<u8>, reqwest_wasm::Error> {
    use futures::executor;

    let response = executor::block_on(reqwest_wasm::get(url))?;
    match response.error_for_status() {
        Ok(r) => {
            let r = executor::block_on(r.bytes())?;
            Ok(r.to_vec())
        }
        Err(e) => Err(e),
    }
}

pub struct Archival<F: FileSystemAPI> {
    fs_mutex: FileSystemMutex<F>,
    // event_receiver: Receiver<ArchivalEvent>,
    // pub event_sender: Sender<ArchivalEvent>,
    site: Site,
}

impl<F: FileSystemAPI> Archival<F> {
    pub fn new(fs: F) -> Self {
        // let (event_sender, event_receiver) = mpsc::channel::<ArchivalEvent>();
        let site = site::load(&fs).unwrap();
        let fs_mutex = FileSystemMutex::init(fs);
        Self {
            fs_mutex,
            // event_receiver,
            // event_sender,
            site,
        }
    }
    pub fn send_event(&self, event: ArchivalEvent) -> Result<(), Box<dyn Error>> {
        match event {
            ArchivalEvent::AddObject(event) => self.add_object(event)?,
            ArchivalEvent::EditField(event) => self.edit_field(event)?,
            ArchivalEvent::EditOrder(event) => self.edit_order(event)?,
        }
        // After any event, rebuild
        site::build(&self.site, &self.fs_mutex)
    }
    // Internal
    fn add_object(&self, event: AddObjectEvent) -> Result<(), Box<dyn Error>> {
        let obj_def = if let Some(o) = self.site.objects.get(&event.object) {
            o
        } else {
            return Err(ArchivalError::new(&format!("object not found: {}", event.object)).into());
        };
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

    fn edit_field(&self, event: EditFieldEvent) -> Result<(), Box<dyn Error>> {
        let obj_def = if let Some(o) = self.site.objects.get(&event.object) {
            o
        } else {
            return Err(ArchivalError::new(&format!("object not found: {}", event.object)).into());
        };
        let mut all_objects = site::get_objects(&self.site, &self.fs_mutex)?;
        let mut existing = if let Some(objects) = all_objects.get_mut(&obj_def.name) {
            if let Some(object) = objects.iter_mut().find(|o| o.filename == event.filename) {
                object
            } else {
                return Err(
                    ArchivalError::new(&format!("filename not found: {}", event.filename)).into(),
                );
            }
        } else {
            return Err(
                ArchivalError::new(&format!("no objects of type: {}", event.object)).into(),
            );
        };
        let path = self
            .site
            .manifest
            .objects_dir
            .join(Path::new(&obj_def.name))
            .join(Path::new(&format!("{}.toml", event.filename)));
        event.path.set_in_object(&mut existing, event.value.into());
        self.fs_mutex
            .with_fs(|fs| fs.write_str(&path, existing.to_toml()?))?;
        Ok(())
    }
    fn edit_order(&self, event: EditOrderEvent) -> Result<(), Box<dyn Error>> {
        let obj_def = if let Some(o) = self.site.objects.get(&event.object) {
            o
        } else {
            return Err(ArchivalError::new(&format!("object not found: {}", event.object)).into());
        };
        let mut all_objects = site::get_objects(&self.site, &self.fs_mutex)?;
        let existing = if let Some(objects) = all_objects.get_mut(&obj_def.name) {
            if let Some(object) = objects.iter_mut().find(|o| o.filename == event.filename) {
                object
            } else {
                return Err(
                    ArchivalError::new(&format!("filename not found: {}", event.filename)).into(),
                );
            }
        } else {
            return Err(
                ArchivalError::new(&format!("no objects of type: {}", event.object)).into(),
            );
        };
        let path = self
            .site
            .manifest
            .objects_dir
            .join(Path::new(&obj_def.name))
            .join(Path::new(&format!("{}.toml", event.filename)));
        existing.order = event.order;
        self.fs_mutex
            .with_fs(|fs| fs.write_str(&path, existing.to_toml()?))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::{file_system::unpack_zip, object::ValuePath};

    use super::*;

    #[test]
    fn load_site_from_zip() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let site = site::load(&fs)?;
        assert_eq!(site.objects.len(), 2);
        assert!(site.objects.contains_key("section"));
        assert!(site.objects.contains_key("post"));
        Ok(())
    }

    #[test]
    fn add_object_to_site() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs);
        archival.send_event(ArchivalEvent::AddObject(AddObjectEvent {
            object: "section".to_string(),
            filename: "my-section".to_string(),
            order: 2,
        }))?;
        // Sending an event should result in an updated fs
        let sections_dir = archival.site.manifest.objects_dir.join(&"section");
        let sections = archival.fs_mutex.with_fs(|fs| fs.read_dir(&sections_dir))?;
        println!("SECTIONS: {:?}", sections);
        assert_eq!(sections.len(), 2);
        let section_toml = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(&sections_dir.join(&"my-section.toml")));
        assert!(section_toml.is_ok());
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(&archival.site.manifest.build_dir.join(&"index.html")))?
            .unwrap();
        let rendered_sections: Vec<_> = index_html.match_indices("<h2>").collect();
        println!("MATCHED: {:?}", rendered_sections);
        assert_eq!(rendered_sections.len(), 2);
        Ok(())
    }

    #[test]
    fn edit_object() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = Archival::new(fs);
        archival.send_event(ArchivalEvent::EditField(EditFieldEvent {
            object: "section".to_string(),
            filename: "first".to_string(),
            path: ValuePath::new().join(object::ValuePathComponent::key("name")),
            value: events::EditFieldValue::String("This is the new title".to_string()),
        }))?;
        // Sending an event should result in an updated fs
        let index_html = archival
            .fs_mutex
            .with_fs(|fs| fs.read_to_string(&archival.site.manifest.build_dir.join(&"index.html")))?
            .unwrap();
        println!("index: {}", index_html);
        assert!(index_html.contains("This is the new title"));
        Ok(())
    }
}

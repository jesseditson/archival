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
pub mod site;
mod tags;
pub use archival_error::ArchivalError;
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
#[cfg(feature = "wasm-fs")]
pub use file_system_wasm::WasmFileSystem;

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

#[cfg(test)]
mod tests {
    use std::{error::Error, path::Path};

    use crate::file_system::unpack_zip;

    use super::*;

    #[test]
    fn load_site_from_zip() -> Result<(), Box<dyn Error>> {
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let site = site::load(Path::new(""), &fs)?;
        assert_eq!(site.objects.len(), 1);
        let first_id = site.objects.keys().next().unwrap();
        assert_eq!(site.objects[first_id].name, "section");
        Ok(())
    }
}

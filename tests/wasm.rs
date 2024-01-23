#[cfg(feature = "wasm-fs")]
mod wasm_tests {
    use std::{error::Error, path::Path};

    use archival::{site, unpack_zip, WasmFileSystem};
    use wasm_bindgen_test::*;
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn wasm_site_from_zip() -> Result<(), Box<dyn Error>> {
        println!("START");
        let downloaded_site = include_bytes!("fixtures/archival-website.zip");
        // let downloaded_site =
        //     fetch_site("https://github.com/jesseditson/archival-website/archive/archival-rs-js.zip")?;
        println!("READ");
        let mut fs = WasmFileSystem::new("archival");
        unpack_zip(downloaded_site.to_vec(), &mut fs)?;
        let site = site::load(Path::new(""), &fs)?;
        assert_eq!(site.objects.len(), 1);
        Ok(())
    }
}

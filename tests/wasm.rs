#[cfg(feature = "wasm-fs")]
mod wasm_tests {
    use std::error::Error;

    use archival::{unpack_zip, Archival, WasmFileSystem};
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
        let _archival = Archival::new(fs);
        // TODO
        Ok(())
    }
}

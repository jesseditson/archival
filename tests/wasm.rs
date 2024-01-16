#[cfg(feature = "fs-wasm")]
mod wasm_tests {
    use archival::WasmFileSystem;
    use wasm_bindgen_test::*;
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn wasm_site() {
        let file_system = WasmFileSystem::new();
    }
}

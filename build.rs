use std::io::Result;
fn main() -> Result<()> {
    #[cfg(feature = "proto")]
    {
        let mut config = prost_build::Config::new();
        // Derive serde for all generated types so we can convert via serde JSON.
        config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
        config.compile_protos(&["proto/archival.proto"], &["proto"])?;
    }
    Ok(())
}

pub const MANIFEST_FILE_NAME: &str = "manifest.toml";
pub const OBJECT_DEFINITION_FILE_NAME: &str = "objects.toml";
pub const PAGES_DIR_NAME: &str = "pages";
pub const OBJECTS_DIR_NAME: &str = "objects";
pub const BUILD_DIR_NAME: &str = "dist";
pub const STATIC_DIR_NAME: &str = "public";
pub const LAYOUT_DIR_NAME: &str = "layout";
pub const CDN_URL: &str = "https://cdn.archival.dev";
#[cfg(debug_assertions)]
#[cfg(feature = "binary")]
pub const AUTH_URL: &str = "http://localhost:8788/cli-login";
#[cfg(not(debug_assertions))]
#[cfg(feature = "binary")]
pub const AUTH_URL: &str = "https://archival.dev/cli-login";

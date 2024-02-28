use std::path::Path;

pub fn as_path_str(string: &str) -> String {
    Path::new(string).to_string_lossy().to_string()
}

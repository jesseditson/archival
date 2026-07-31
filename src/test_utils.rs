use crate::util::path_to_native;
use std::path::Path;

/// Rewrites a `/`-separated path literal into the platform's native form, so
/// tests can compare hardcoded paths against paths a filesystem produced.
/// `Path::new` does not do this on windows - it accepts `/` as a separator but
/// preserves it when converting back to a string.
pub fn as_path_str(string: &str) -> String {
    path_to_native(Path::new(string))
        .to_string_lossy()
        .to_string()
}

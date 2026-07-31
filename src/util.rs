use std::borrow::Cow;
use std::path::{Path, PathBuf, MAIN_SEPARATOR, MAIN_SEPARATOR_STR};

/// Renders a path using `/` separators regardless of the host platform. Use
/// this for anything that leaves archival as a web path: liquid variables,
/// template/partial names, urls. Paths that are handed back to a filesystem
/// should keep their native separators instead.
pub fn path_to_slash(path: impl AsRef<Path>) -> String {
    let path = path.as_ref().to_string_lossy();
    if MAIN_SEPARATOR == '/' {
        path.into_owned()
    } else {
        path.replace(MAIN_SEPARATOR, "/")
    }
}

/// Rewrites a path so that it uses the platform's native separator. Windows
/// usually accepts `/` in paths, but verbatim paths (`\\?\C:\...`, which is
/// what `fs::canonicalize` returns) do not, and `Path::join` never translates
/// separators - so a `/` in a joined path can survive all the way to the OS.
pub fn path_to_native(path: &Path) -> Cow<'_, Path> {
    if MAIN_SEPARATOR == '/' {
        Cow::Borrowed(path)
    } else {
        Cow::Owned(PathBuf::from(
            path.to_string_lossy().replace('/', MAIN_SEPARATOR_STR),
        ))
    }
}

pub fn integer_decode(val: f64) -> (u64, i16, i8) {
    let bits: u64 = val.to_bits();
    let sign: i8 = if bits >> 63 == 0 { 1 } else { -1 };
    let mut exponent: i16 = ((bits >> 52) & 0x7ff) as i16;
    let mantissa = if exponent == 0 {
        (bits & 0xfffffffffffff) << 1
    } else {
        (bits & 0xfffffffffffff) | 0x10000000000000
    };

    exponent -= 1023 + 52;
    (mantissa, exponent, sign)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_render_with_forward_slashes() {
        let native: PathBuf = ["artist", "tormenta-rey"].iter().collect();
        assert_eq!(path_to_slash(&native), "artist/tormenta-rey");
    }

    #[test]
    fn paths_convert_to_native_separators() {
        let native: PathBuf = ["pages", "post", "single.liquid"].iter().collect();
        assert_eq!(
            path_to_native(Path::new("pages/post/single.liquid")).as_ref(),
            native.as_path()
        );
    }
}

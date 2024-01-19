use std::{
    error::Error,
    path::{Path, PathBuf}, io::{Read, Seek, Cursor},
};

pub trait FileSystemAPI {
    fn remove_dir_all(&mut self, path: &Path) -> Result<(), Box<dyn Error>>;
    fn create_dir_all(&mut self, path: &Path) -> Result<(), Box<dyn Error>>;
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>>;
    fn read_to_string(&self, path: &Path) -> Result<Option<String>, Box<dyn Error>>;
    fn write(&mut self, path: &Path, contents: Vec<u8>) -> Result<(), Box<dyn Error>>;
    fn write_str(&mut self, path: &Path, contents: String) -> Result<(), Box<dyn Error>>;
    fn copy_contents(&mut self, from: &Path, to: &Path) -> Result<(), Box<dyn Error>>;
    fn walk_dir(&self, path: &Path) -> Result<Box<dyn Iterator<Item = PathBuf>>, Box<dyn Error>>;
}

pub trait WatchableFileSystemAPI {
    fn watch(
        &mut self,
        root: PathBuf,
        watch_paths: Vec<String>,
        changed: impl Fn(Vec<PathBuf>) + Send + Sync + 'static,
    ) -> Result<Box<dyn FnOnce() + '_>, Box<dyn Error>>;
}

fn has_toplevel<S: Read + Seek>(
    archive: &mut zip::ZipArchive<S>,
) -> Result<bool, zip::result::ZipError> {
    let mut toplevel_dir: Option<PathBuf> = None;
    if archive.len() < 2 {
        return Ok(false);
    }

    for i in 0..archive.len() {
        let file = archive.by_index(i)?.mangled_name();
        if let Some(toplevel_dir) = &toplevel_dir {
            if !file.starts_with(toplevel_dir) {
                return Ok(false);
            }
        } else {
            // First iteration
            let comp: PathBuf = file.components().take(1).collect();
            toplevel_dir = Some(comp);
        }
    }
    Ok(true)
}

pub fn unpack_zip(zipball: Vec<u8>, fs: &mut impl FileSystemAPI) -> Result<(), Box<dyn Error>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zipball))?;

    let do_strip_toplevel = has_toplevel(&mut archive)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let mut relative_path = file.mangled_name();

        if do_strip_toplevel {
            let base = relative_path
                .components()
                .take(1)
                .fold(PathBuf::new(), |mut p, c| {
                    p.push(c);
                    p
                });
            relative_path = relative_path.strip_prefix(&base)?.to_path_buf()
        }

        if relative_path.to_string_lossy().is_empty() {
            // Top-level directory
            continue;
        }

        let mut outpath = PathBuf::new();
        outpath.push(relative_path);

        if file.name().ends_with('/') {
            fs.create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs.create_dir_all(&p)?;
                }
            }
            let mut buffer = vec![];
            file.read_to_end(&mut buffer)?;
            fs.write(&outpath, buffer)?;
        }
    }
    Ok(())
}
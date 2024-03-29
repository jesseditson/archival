mod tests {
    use std::{
        error::Error,
        path::{Path, PathBuf},
    };

    use crate::{file_system::unpack_zip, test_utils::as_path_str, FileSystemAPI};

    pub fn write_and_read_files(mut fs: impl FileSystemAPI) -> Result<(), Box<dyn Error>> {
        let dir = Path::new("some/deep/path");
        let file = dir.join("myfile.txt");
        let content = "foo bar baz".to_string();
        fs.create_dir_all(dir)?;
        println!("write: {}", file.as_path().display());
        fs.write_str(file.as_path(), content.to_owned())?;
        let files = fs.walk_dir(Path::new(""), false)?.collect::<Vec<PathBuf>>();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], file);
        let content_o = fs.read_to_string(file.as_path())?;
        assert!(content_o.is_some());
        assert_eq!(content, content_o.unwrap());
        fs.delete(file.as_path())?;
        let files = fs.walk_dir(Path::new(""), false)?.collect::<Vec<PathBuf>>();
        println!("files: {:?}", files);
        assert_eq!(files.len(), 0);
        let file = dir.join("myfile.bin");
        let content: Vec<u8> = vec![0, 3, 4];
        fs.write(file.as_path(), content.clone())?;
        let content_o = fs.read(file.as_path())?;
        assert_eq!(content_o.unwrap(), content);
        Ok(())
    }

    pub fn unzip_to_fs(mut fs: impl FileSystemAPI) -> Result<(), Box<dyn Error>> {
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let ob_def = fs.read_to_string(Path::new("objects.toml"))?;
        assert!(ob_def.is_some());
        let files = fs
            .walk_dir(Path::new(""), false)?
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<String>>();
        println!("files: {:?}", files);
        assert!(files.contains(&as_path_str("layout/theme.liquid")));
        assert!(!files.contains(&"layout".to_string()));
        Ok(())
    }
}

macro_rules! gen_test {
    ($test_fn: ident, $fs_impl:expr) => {
        #[test]
        fn $test_fn() -> Result<(), Box<dyn std::error::Error>> {
            tests::$test_fn($fs_impl)?;
            Ok(())
        }
    };
}

mod memory {
    use crate::file_system_memory;

    use super::tests;
    fn get_fs() -> file_system_memory::MemoryFileSystem {
        file_system_memory::MemoryFileSystem::default()
    }
    gen_test!(write_and_read_files, get_fs());
    gen_test!(unzip_to_fs, get_fs());
}

#[cfg(feature = "stdlib-fs")]
mod stdlib {
    use rand::{distributions::Alphanumeric, Rng};
    use std::{fs, path::Path};

    use crate::file_system_stdlib;

    use super::tests;
    fn get_fs() -> file_system_stdlib::NativeFileSystem {
        let rand_dir: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(14)
            .map(char::from)
            .collect();
        let dir = format!("./target/file-system-tests/{}", rand_dir);
        fs::create_dir_all(&dir).unwrap();
        println!("DIR: {}", dir);
        file_system_stdlib::NativeFileSystem::new(Path::new(&dir))
    }
    gen_test!(write_and_read_files, get_fs());
    gen_test!(unzip_to_fs, get_fs());
}

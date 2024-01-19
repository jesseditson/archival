mod tests {
    use std::{error::Error, path::Path};

    use crate::{unpack_zip, FileSystemAPI, WatchableFileSystemAPI};

    pub fn write_and_read_files(mut fs: impl FileSystemAPI) -> Result<(), Box<dyn Error>> {
        let dir = Path::new("some/deep/path");
        let file = dir.join("myfile.txt");
        let content = "foo bar baz".to_string();
        fs.create_dir_all(dir)?;
        fs.write_str(file.as_path(), content.to_owned())?;
        let files = fs.read_dir(dir)?;
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], file);
        let content_o = fs.read_to_string(file.as_path())?;
        assert!(content_o.is_some());
        assert_eq!(content, content_o.unwrap());
        Ok(())
    }

    pub fn unzip_to_fs(
        mut fs: impl FileSystemAPI + WatchableFileSystemAPI,
    ) -> Result<(), Box<dyn Error>> {
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let dirs = fs.read_dir(Path::new("/"))?;
        assert_eq!(dirs.len(), 19);
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
        file_system_memory::MemoryFileSystem::new()
    }
    gen_test!(write_and_read_files, get_fs());
    // gen_test!(unzip_to_fs, get_fs());
}

#[cfg(feature = "stdlib-fs")]
mod stdlib {
    use std::{env, fs};

    use crate::file_system_stdlib;

    use super::tests;
    fn get_fs() -> file_system_stdlib::NativeFileSystem {
        fs::remove_dir_all("target/file-system-tests");
        fs::create_dir_all("target/file-system-tests");
        env::set_current_dir("target/file-system-tests");
        file_system_stdlib::NativeFileSystem
    }
    gen_test!(write_and_read_files, get_fs());
    gen_test!(unzip_to_fs, get_fs());
}

#[cfg(feature = "wasm-fs")]
mod wasm {
    use crate::file_system_wasm;

    use super::tests;
    fn get_fs() -> file_system_wasm::WasmFileSystem {
        file_system_wasm::WasmFileSystem::new("test")
    }
    gen_test!(write_and_read_files, get_fs());
    gen_test!(unzip_to_fs, get_fs());
}

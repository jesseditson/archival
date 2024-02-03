mod tests {
    use std::{error::Error, path::Path};

    use crate::{file_system::unpack_zip, FileSystemAPI, WatchableFileSystemAPI};

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
        fs.delete(file.as_path())?;
        let files = fs.read_dir(dir)?;
        assert_eq!(files.len(), 0);
        Ok(())
    }

    pub fn unzip_to_fs(
        mut fs: impl FileSystemAPI + WatchableFileSystemAPI,
    ) -> Result<(), Box<dyn Error>> {
        let zip = include_bytes!("../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        println!("files: {:?}", fs.read_dir(Path::new(""))?);
        let root_files: Vec<String> = fs
            .read_dir(Path::new(""))?
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_owned())
            .collect();
        let ob_def = fs.read_to_string(Path::new("objects.toml"))?;
        assert!(root_files.contains(&"layout".to_string()));
        assert!(ob_def.is_some());
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
    use std::{env, fs, path::Path};

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
        env::set_current_dir(&dir).unwrap();
        file_system_stdlib::NativeFileSystem::new(Path::new(&dir))
    }
    gen_test!(write_and_read_files, get_fs());
    gen_test!(unzip_to_fs, get_fs());
}

// #[cfg(feature = "wasm-fs")]
// mod wasm {
//     use crate::file_system_wasm;

//     use super::tests;
//     fn get_fs() -> file_system_wasm::WasmFileSystem {
//         file_system_wasm::WasmFileSystem::new("test")
//     }
//     gen_test!(write_and_read_files, get_fs());
//     gen_test!(unzip_to_fs, get_fs());
// }

#[cfg(feature = "binary")]
mod binary_tests {
    use std::{error::Error, fs, path::Path};

    use walkdir::WalkDir;

    fn get_args(args: Vec<&str>) -> impl Iterator<Item = String> {
        let mut a = vec!["archival".to_string()];
        for arg in args {
            a.push(arg.to_string())
        }
        a.into_iter()
    }

    #[test]
    fn build_basics() -> Result<(), Box<dyn Error>> {
        _ = fs::remove_dir_all("tests/fixtures/website/dist");
        assert!(Path::new("tests/fixtures/website").exists());
        println!(
            "files: {:?}",
            WalkDir::new(Path::new("tests/fixtures/website"))
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
        );
        archival::binary::binary(get_args(vec!["build", "tests/fixtures/website"]))?;
        Ok(())
    }
    #[test]
    fn run_watcher() -> Result<(), Box<dyn Error>> {
        // TODO: spawn a thread and send sigint
        // archival::binary(get_args(vec!["run", "tests/fixtures/website"]))?;
        Ok(())
    }
}

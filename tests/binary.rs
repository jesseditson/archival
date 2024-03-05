#[cfg(feature = "binary")]
mod binary_tests {
    use std::{error::Error, fs, path::Path};

    use archival::binary::command::ExitStatus;
    use tracing_test::traced_test;
    use walkdir::WalkDir;

    fn get_args(args: Vec<&str>) -> impl Iterator<Item = String> {
        let mut a = vec!["archival".to_string()];
        for arg in args {
            a.push(arg.to_string())
        }
        a.into_iter()
    }

    #[test]
    #[traced_test]
    fn build_basics() -> Result<(), Box<dyn Error>> {
        _ = fs::remove_dir_all("tests/fixtures/website/dist");
        assert!(Path::new("tests/fixtures/website").exists());
        println!("current dir: {}", std::env::current_dir()?.display());
        println!(
            r"files: \n{}",
            WalkDir::new(Path::new("tests/fixtures/website"))
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .map(|de| de.into_path().to_string_lossy().to_string())
                .collect::<Vec<String>>()
                .join("\n")
        );
        archival::binary::binary(get_args(vec!["build", "tests/fixtures/website"]))?;
        Ok(())
    }

    #[test]
    #[traced_test]
    fn run_watcher() -> Result<(), Box<dyn Error>> {
        // TODO: spawn a thread and send sigint
        // archival::binary(get_args(vec!["run", "tests/fixtures/website"]))?;
        Ok(())
    }

    #[test]
    #[traced_test]
    fn compatiblity_ok() -> Result<(), Box<dyn Error>> {
        assert!(matches!(
            archival::binary::binary(get_args(vec!["compat", "0.4.1"]))?,
            ExitStatus::Ok
        ));
        Ok(())
    }

    #[test]
    #[traced_test]
    fn compatiblity_not_ok() -> Result<(), Box<dyn Error>> {
        assert!(matches!(
            archival::binary::binary(get_args(vec!["compat", "0.1.1"]))?,
            ExitStatus::Error
        ));
        Ok(())
    }
}

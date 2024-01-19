#[cfg(feature = "binary")]
mod binary_tests {
    use archival;
    use std::{env, error::Error, fs};

    fn get_args(args: Vec<&str>) -> impl Iterator<Item = String> {
        let mut a = vec!["archival".to_string()];
        for arg in args {
            a.push(arg.to_string())
        }
        a.into_iter()
    }

    #[test]
    fn build_basics() -> Result<(), Box<dyn Error>> {
        fs::create_dir_all("target/tests");
        env::set_current_dir("target/tests");
        archival::binary(get_args(vec!["build", "tests/fixtures/website"]))?;
        Ok(())
    }
    #[test]
    fn run_watcher() -> Result<(), Box<dyn Error>> {
        // TODO: spawn a thread and send sigint
        // archival::binary(get_args(vec!["run", "tests/fixtures/website"]))?;
        Ok(())
    }
}

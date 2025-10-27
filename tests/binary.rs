#[cfg(feature = "binary")]
mod binary_tests {
    use std::{
        error::Error,
        fs,
        io::Read,
        path::Path,
        process::{Command, Stdio},
        sync, thread,
        time::{Duration, Instant},
    };

    use archival::binary::command::ExitStatus;
    use nanoid::nanoid;
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
    fn build_basics() {
        _ = fs::remove_dir_all("tests/fixtures/website/dist");
        assert!(Path::new("tests/fixtures/website").exists());
        println!(
            "current dir: {}",
            std::env::current_dir().unwrap().display()
        );
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
        archival::binary::binary(
            get_args(vec![
                "build",
                "tests/fixtures/website",
                "--upload-prefix",
                "test",
            ]),
            None,
        )
        .unwrap();
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
            archival::binary::binary(get_args(vec!["compat", "0.10.0"]), None)?,
            ExitStatus::Ok
        ));
        Ok(())
    }

    #[test]
    #[traced_test]
    fn compatiblity_not_ok() -> Result<(), Box<dyn Error>> {
        assert!(matches!(
            archival::binary::binary(get_args(vec!["compat", "0.1.1"]), None)?,
            ExitStatus::Error
        ));
        Ok(())
    }
    static SUBPAGE_CONTENT: &str = r#"
        name="HI"
        "#;
    #[test]
    #[traced_test]
    fn run_removes_files_when_objects_deleted() {
        _ = fs::remove_dir_all("tests/fixtures/website/dist");
        assert!(Path::new("tests/fixtures/website").exists());
        _ = fs::create_dir("tests/fixtures/tmp");
        let site_path = format!("tests/fixtures/tmp/{}", nanoid!());
        copy_dir_all("tests/fixtures/website", &site_path).unwrap();

        println!(
            "current dir: {}",
            std::env::current_dir().unwrap().display()
        );
        println!(
            r"files: \n{}",
            WalkDir::new(&site_path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .map(|de| de.into_path().to_string_lossy().to_string())
                .collect::<Vec<String>>()
                .join("\n")
        );
        let mut run_cmd = Command::new("cargo")
            .args(["run", "run", &site_path, "--upload-prefix", "test"])
            .current_dir(std::env::current_dir().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stream = run_cmd.stdout.take().unwrap();
        let (sender, receiver) = sync::mpsc::channel();
        thread::spawn(move || loop {
            let mut buf = [0];
            match stream.read(&mut buf) {
                Err(err) => {
                    panic!("{}] Error reading from stream: {}", line!(), err);
                }
                Ok(len) => {
                    if len > 0 {
                        sender.send(buf).expect("send failed");
                    }
                }
            }
        });
        // This takes long enough that we don't need to orchestrate via the callback
        let s = run_until(&receiver, "Serving", Duration::from_millis(5000), || {});
        println!("-------- initial build: {}", String::from_utf8_lossy(&s));
        // First build complete. Now add a file and make sure it's built
        let spid = nanoid!();
        let new_section_path = format!("{}/objects/subpage/{}.toml", site_path, spid);
        println!("LOOKING FOR NEW PAGE...");
        let s = run_until(&receiver, "Rebuilt", Duration::from_millis(2000), || {
            fs::write(&new_section_path, SUBPAGE_CONTENT).unwrap();
        });
        println!("-------- new page: {}", String::from_utf8_lossy(&s));
        let new_section_page_path = format!("{}/dist/subpage/{}.html", site_path, spid);
        let found_page = fs::exists(&new_section_page_path).unwrap();
        assert!(found_page);
        // Now remove the object and verify that the page was also removed;
        let s = run_until(&receiver, "Rebuilt", Duration::from_millis(2000), || {
            fs::remove_file(&new_section_path).unwrap();
        });
        println!("-------- deleted: {}", String::from_utf8_lossy(&s));
        let found_page = fs::exists(&new_section_page_path).unwrap();
        assert!(!found_page);
        run_cmd.kill().unwrap();
        run_cmd.wait().unwrap();
        // _ = fs::remove_dir_all(site_path);
    }

    fn run_until(
        receiver: &sync::mpsc::Receiver<[u8; 1]>,
        until_seen: &str,
        timeout: Duration,
        f: impl FnOnce(),
    ) -> Vec<u8> {
        let start = Instant::now();
        let mut current_buf = vec![];
        f();
        loop {
            if Instant::now() - start > timeout {
                panic!("timed out waiting for {}", until_seen);
            }
            match receiver.try_recv() {
                Ok(bytes) => {
                    current_buf.append(&mut bytes.to_vec());
                    let str = String::from_utf8_lossy(&current_buf);
                    if str.contains(until_seen) {
                        break;
                    }
                }
                Err(sync::mpsc::TryRecvError::Empty) => {}
                Err(sync::mpsc::TryRecvError::Disconnected) => panic!("Channel disconnected"),
            }
        }
        current_buf
    }

    fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
        fs::create_dir_all(&dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_dir() {
                copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
            } else {
                fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
            }
        }
        Ok(())
    }
}

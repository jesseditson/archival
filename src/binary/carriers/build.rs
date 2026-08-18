//! Installing and building carriers, mirroring what archival-editor's deploy
//! container does before it hands a carrier to wrangler.

use super::discovery::Carrier;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tracing::debug;

/// `npm` on unix is a shell script; on windows only `npm.cmd` is executable by
/// `CreateProcess`.
fn npm() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

/// What an install depends on. Recording it means restarting `archival run`
/// doesn't reinstall a carrier whose dependencies never changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Fingerprints(HashMap<String, u64>);

impl Fingerprints {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Ok(json) = serde_json::to_string(self) {
            let _ = fs::write(path, json);
        }
    }

    pub fn is_current(&self, carrier: &Carrier) -> bool {
        // A carrier with no package.json has nothing to install, so it is
        // always current. One whose node_modules went away is not, whatever
        // the recorded hash says.
        if carrier.package.is_none() {
            return true;
        }
        if !carrier.root.join("node_modules").is_dir() {
            return false;
        }
        self.0.get(&carrier.name) == Some(&install_fingerprint(carrier))
    }

    pub fn record(&mut self, carrier: &Carrier) {
        self.0
            .insert(carrier.name.to_owned(), install_fingerprint(carrier));
    }
}

fn install_fingerprint(carrier: &Carrier) -> u64 {
    let mut bytes = fs::read(carrier.root.join("package.json")).unwrap_or_default();
    if let Some(lockfile) = &carrier.lockfile {
        bytes.extend(fs::read(lockfile).unwrap_or_default());
    }
    seahash::hash(&bytes)
}

/// Installs dependencies and runs the carrier's `build` script, if it has
/// either. Returns the entry file to import.
pub(crate) fn prepare(carrier: &Carrier, fingerprints: &mut Fingerprints) -> Result<PathBuf> {
    if carrier.package.is_some() && !fingerprints.is_current(carrier) {
        let install = if carrier.lockfile.is_some() {
            vec!["ci", "--audit=false"]
        } else {
            vec!["install", "--audit=false"]
        };
        run(carrier, &install)?;
        fingerprints.record(carrier);
    }
    if carrier.package.as_ref().is_some_and(|p| p.has_build_script) {
        run(carrier, &["run", "build"])?;
    }
    // Resolved after install so a package's install hooks can generate it.
    carrier.entry().ok_or_else(|| {
        anyhow!(
            "couldn't find a main file for carrier `{}` in {} - expected {}",
            carrier.name,
            carrier.root.display(),
            carrier.expected_entry()
        )
    })
}

fn run(carrier: &Carrier, args: &[&str]) -> Result<()> {
    debug!("carrier {}: npm {}", carrier.name, args.join(" "));
    let output = Command::new(npm())
        .args(args)
        .current_dir(&carrier.root)
        .output()
        .map_err(|e| {
            anyhow!(
                "couldn't run `{} {}` for carrier `{}`: {}",
                npm(),
                args.join(" "),
                carrier.name,
                e
            )
        })?;
    if output.status.success() {
        return Ok(());
    }
    let mut detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if detail.is_empty() {
        detail = String::from_utf8_lossy(&output.stdout).trim().to_string();
    }
    Err(anyhow!(
        "`npm {}` failed for carrier `{}`:\n{}",
        args.join(" "),
        carrier.name,
        detail
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::carriers::discovery::{discover, PackageJson};
    use std::fs::{create_dir_all, write};
    use tempfile::TempDir;

    fn site(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for (path, contents) in files {
            let path = dir.path().join(path);
            create_dir_all(path.parent().unwrap()).unwrap();
            write(path, contents).unwrap();
        }
        dir
    }

    #[test]
    fn a_carrier_with_no_package_never_installs() {
        let dir = site(&[("carriers/echo/index.js", "")]);
        let carrier = &discover(dir.path()).unwrap()[0];
        assert_eq!(carrier.package, None);
        assert!(Fingerprints::default().is_current(carrier));
    }

    #[test]
    fn a_package_without_node_modules_is_never_current() {
        let dir = site(&[
            ("carriers/echo/index.js", ""),
            ("carriers/echo/package.json", "{}"),
        ]);
        let carrier = &discover(dir.path()).unwrap()[0];
        let mut fingerprints = Fingerprints::default();
        fingerprints.record(carrier);
        assert!(
            !fingerprints.is_current(carrier),
            "a recorded hash does not make up for a missing node_modules"
        );
    }

    #[test]
    fn a_changed_lockfile_invalidates_the_install() {
        let dir = site(&[
            ("carriers/echo/index.js", ""),
            ("carriers/echo/package.json", "{}"),
            ("carriers/echo/package-lock.json", r#"{"v":1}"#),
            ("carriers/echo/node_modules/.keep", ""),
        ]);
        let carrier = &discover(dir.path()).unwrap()[0];
        assert_eq!(
            carrier.package,
            Some(PackageJson {
                main: None,
                has_build_script: false
            })
        );
        let mut fingerprints = Fingerprints::default();
        fingerprints.record(carrier);
        assert!(fingerprints.is_current(carrier));

        write(
            dir.path().join("carriers/echo/package-lock.json"),
            r#"{"v":2}"#,
        )
        .unwrap();
        let carrier = &discover(dir.path()).unwrap()[0];
        assert!(!fingerprints.is_current(carrier));
    }

    #[test]
    fn fingerprints_survive_a_round_trip_to_disk() {
        let dir = site(&[
            ("carriers/echo/index.js", ""),
            ("carriers/echo/package.json", "{}"),
            ("carriers/echo/node_modules/.keep", ""),
        ]);
        let carrier = &discover(dir.path()).unwrap()[0];
        let mut fingerprints = Fingerprints::default();
        fingerprints.record(carrier);
        let path = dir.path().join("fingerprints.json");
        fingerprints.save(&path);
        assert!(Fingerprints::load(&path).is_current(carrier));
    }

    #[test]
    fn a_missing_fingerprint_file_loads_as_empty() {
        assert_eq!(
            Fingerprints::load(Path::new("/nonexistent/fingerprints.json")),
            Fingerprints::default()
        );
    }
}

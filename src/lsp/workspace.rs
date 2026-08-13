//! Finding the archival site a file belongs to, and keeping its definitions
//! loaded.

use crate::constants::{MANIFEST_FILE_NAME, OBJECT_DEFINITION_FILE_NAME};
use crate::file_system_stdlib::NativeFileSystem;
use crate::object_definition::ObjectDefinition;
use crate::site::Site;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Either of these at the root of a directory makes it an archival site. The
/// legacy `manifest.toml` and `objects.toml` names are not recognized.
const MARKERS: [&str; 2] = [MANIFEST_FILE_NAME, OBJECT_DEFINITION_FILE_NAME];

pub(crate) struct SiteState {
    pub root: PathBuf,
    /// `None` when the site is present but unreadable, which leaves features
    /// that need definitions switched off rather than the server stopped.
    pub site: Option<Site>,
}

impl SiteState {
    fn load(root: PathBuf) -> Self {
        let fs = NativeFileSystem::new(&root);
        // The upload prefix only affects file field urls, which nothing here
        // reads.
        let site = match Site::load(&fs, Some("")) {
            Ok(site) => Some(site),
            Err(err) => {
                warn!("{}: could not load site: {err}", root.display());
                None
            }
        };
        Self { root, site }
    }

    /// Whether `file` is this site's `archival.toml`.
    pub fn is_manifest(&self, file: &Path) -> bool {
        file == self.root.join(MANIFEST_FILE_NAME)
    }

    /// Whether `file` is this site's object definitions. The name is
    /// configurable through the manifest's `object_file`, so a site that failed
    /// to load falls back to the default.
    pub fn is_definitions(&self, file: &Path) -> bool {
        let named = self
            .site
            .as_ref()
            .map(|site| self.root.join(&site.manifest.object_definition_file));
        match named {
            Some(named) => file == named,
            None => file == self.root.join(OBJECT_DEFINITION_FILE_NAME),
        }
    }

    /// This site's manifest, when it loaded.
    pub fn manifest(&self) -> Option<&crate::manifest::Manifest> {
        self.site.as_ref().map(|site| &site.manifest)
    }

    /// The definition an object file is an instance of, given its path.
    ///
    /// Objects live at `<objects_dir>/<name>.toml` when there is one of them,
    /// and `<objects_dir>/<name>/<anything>.toml` when there are many.
    pub fn definition_for(&self, file: &Path) -> Option<&ObjectDefinition> {
        let site = self.site.as_ref()?;
        // Manifest paths are relative to the site root, because the filesystem
        // the site was loaded through is rooted there.
        let relative = file
            .strip_prefix(&self.root)
            .ok()?
            .strip_prefix(&site.manifest.objects_dir)
            .ok()?;
        let mut parts = relative.components();
        let first = parts.next()?.as_os_str().to_str()?;
        let name = if parts.next().is_some() {
            first.to_string()
        } else {
            first.strip_suffix(".toml")?.to_string()
        };
        site.object_definitions.get(&name)
    }
}

#[derive(Default)]
pub(crate) struct Workspace {
    sites: Vec<SiteState>,
    /// Directories already searched for a site root, so that a file outside any
    /// site is not re-resolved on every keystroke.
    resolved: HashMap<PathBuf, Option<usize>>,
}

impl Workspace {
    /// The site `file` belongs to, loading it the first time it is needed.
    pub fn site_for(&mut self, file: &Path) -> Option<&SiteState> {
        let dir = file.parent()?;
        let index = match self.resolved.get(dir) {
            Some(found) => *found,
            None => {
                let found = self.find_or_load(dir);
                self.resolved.insert(dir.to_path_buf(), found);
                found
            }
        };
        index.map(|index| &self.sites[index])
    }

    fn find_or_load(&mut self, dir: &Path) -> Option<usize> {
        let root = site_root(dir)?;
        if let Some(index) = self.sites.iter().position(|site| site.root == root) {
            return Some(index);
        }
        debug!("loading site at {}", root.display());
        self.sites.push(SiteState::load(root));
        Some(self.sites.len() - 1)
    }

    /// Drops every loaded site, so the next lookup reads from disk again.
    /// Definitions are only read at load, so a change to them needs a reload
    /// rather than an invalidation.
    pub fn reload(&mut self) {
        self.sites.clear();
        self.resolved.clear();
    }

    /// Whether `file` is one whose contents the loaded sites were built from.
    pub fn is_definition_file(&self, file: &Path) -> bool {
        file.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| MARKERS.contains(&name))
    }
}

/// The nearest ancestor of `dir` (including itself) that looks like a site.
fn site_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|candidate| MARKERS.iter().any(|marker| candidate.join(marker).exists()))
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/website")
    }

    #[test]
    fn finds_the_site_a_file_belongs_to() {
        let mut workspace = Workspace::default();
        let site = workspace
            .site_for(&fixture().join("pages/index.liquid"))
            .expect("no site found");
        assert_eq!(site.root, fixture());
        assert!(site.site.is_some());
    }

    #[test]
    fn reports_no_site_outside_one() {
        let mut workspace = Workspace::default();
        assert!(workspace
            .site_for(Path::new("/tmp/nowhere/a.liquid"))
            .is_none());
    }

    #[test]
    fn maps_object_files_to_their_definitions() {
        let mut workspace = Workspace::default();
        let root = fixture();
        let site = workspace
            .site_for(&root.join("pages/index.liquid"))
            .unwrap();
        // Absolute, as every path the server sees is.
        let objects = root.join("objects");

        // A directory of objects.
        let listed = site.definition_for(&objects.join("section/one.toml"));
        assert_eq!(listed.map(|d| &d.name[..]), Some("section"));
        // A single root object.
        let single = site.definition_for(&objects.join("section.toml"));
        assert_eq!(single.map(|d| &d.name[..]), Some("section"));
        // Not an object at all.
        assert!(site
            .definition_for(&root.join("pages/index.liquid"))
            .is_none());
        assert!(site
            .definition_for(&objects.join("nonexistent/a.toml"))
            .is_none());
    }
}

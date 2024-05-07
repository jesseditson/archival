use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    hash::Hash,
    ops::Deref,
    path::{Path, PathBuf},
};
use tracing::debug;

use crate::ArchivalError;

use super::FileSystemAPI;

#[derive(Debug, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    path: PathBuf,
    is_file: bool,
}

impl PartialEq for DirEntry {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Hash for DirEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl DirEntry {
    fn new(path: &Path, is_file: bool) -> Self {
        Self {
            path: path.to_path_buf(),
            is_file,
        }
    }
    fn copy(&self) -> Self {
        Self {
            path: self.path.to_owned(),
            is_file: self.is_file,
        }
    }
}

impl Deref for DirEntry {
    type Target = PathBuf;
    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileGraphNode {
    path: PathBuf,
    pub(crate) files: HashSet<DirEntry>,
}

impl FileGraphNode {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            files: HashSet::new(),
        }
    }
    pub fn key(path: &Path) -> String {
        path.to_string_lossy().to_lowercase()
    }
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
    pub fn add(&mut self, path: &Path, is_file: bool) {
        self.files.insert(DirEntry::new(path, is_file));
    }
    pub fn remove(&mut self, path: &Path) {
        // Since we impl PartialEq, is_file doesn't matter for comparison
        self.files.remove(&DirEntry::new(path, false));
    }
    pub fn copy(&self) -> Self {
        Self {
            path: self.path.to_path_buf(),
            files: self.files.iter().map(|r| r.copy()).collect(),
        }
    }
}

#[derive(Default)]
pub struct MemoryFileSystem {
    fs: HashMap<String, Vec<u8>>,
    tree: HashMap<String, FileGraphNode>,
}

impl FileSystemAPI for MemoryFileSystem {
    fn exists(&self, path: &Path) -> Result<bool, Box<dyn Error>> {
        if self.fs.contains_key(&path.to_string_lossy().to_lowercase()) || self.is_dir(path)? {
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn is_dir(&self, path: &Path) -> Result<bool, Box<dyn Error>> {
        Ok(self.tree.contains_key(&FileGraphNode::key(path)))
    }
    fn remove_dir_all(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        self.remove_from_graph(path);
        Ok(())
    }
    fn create_dir_all(&mut self, _path: &Path) -> Result<(), Box<dyn Error>> {
        // dirs are implicitly created when files are created in them
        Ok(())
    }
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        Ok(self.read_file(path))
    }
    fn read_to_string(&self, path: &Path) -> Result<Option<String>, Box<dyn Error>> {
        if let Some(file) = self.read_file(path) {
            Ok(Some(std::str::from_utf8(&file)?.to_string()))
        } else {
            Ok(None)
        }
    }
    fn delete(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        if self.is_dir(path)? {
            return Err(ArchivalError::new("use remove_dir_all to delete directories").into());
        }
        self.delete_file(path);
        Ok(())
    }
    fn write(&mut self, path: &Path, contents: Vec<u8>) -> Result<(), Box<dyn Error>> {
        if self.is_dir(path)? {
            return Err(ArchivalError::new("cannot write to a folder").into());
        }
        self.write_file(path, contents);
        Ok(())
    }
    fn write_str(&mut self, path: &Path, contents: String) -> Result<(), Box<dyn Error>> {
        self.write(path, contents.as_bytes().to_vec())
    }
    fn copy_recursive(&mut self, from: &Path, to: &Path) -> Result<(), Box<dyn Error>> {
        debug!("copy {} -> {}", from.display(), to.display());
        let mut changed_paths = vec![];
        if !self.is_dir(from)? {
            if let Some(file) = self.read_file(from) {
                self.write_file(to, file);
                changed_paths.push(to.to_path_buf());
            }
        } else {
            for child in self.walk_dir(from, true)? {
                let dest = to.join(&child);
                debug!("copy {} -> {}", from.join(&child).display(), dest.display());
                if let Some(file) = self.read_file(&from.join(child)) {
                    self.write_file(&dest, file);
                    changed_paths.push(to.to_path_buf());
                }
            }
        }
        Ok(())
    }
    fn walk_dir(
        &self,
        path: &Path,
        include_dirs: bool,
    ) -> Result<Box<dyn Iterator<Item = PathBuf>>, Box<dyn Error>> {
        let path = path.to_path_buf();
        let children = self.all_children(&path);
        let mut all_files: Vec<PathBuf> = vec![];
        for child in children {
            let node = self.get_node(&child);
            all_files.append(
                &mut node
                    .files
                    .into_iter()
                    .filter_map(|de| {
                        if !include_dirs && !de.is_file {
                            return None;
                        }
                        Some(de.path.strip_prefix(&path).unwrap().to_owned())
                    })
                    .collect(),
            );
        }
        Ok(Box::new(all_files.into_iter()))
    }
}

impl MemoryFileSystem {
    fn write_file(&mut self, path: &Path, data: Vec<u8>) {
        debug!("write: {}", path.display());
        self.fs.insert(path.to_string_lossy().to_lowercase(), data);
        self.write_to_graph(path, true);
    }

    fn write_to_graph(&mut self, path: &Path, is_file: bool) {
        // Traverse up the path and add each file to its parent's node
        let mut last_path: PathBuf = PathBuf::new();
        let mut is_file = is_file;
        for ancestor in path.ancestors() {
            let a_path = ancestor.to_path_buf();
            // Skip the actual file path, since only directories are nodes
            if a_path.to_string_lossy() != path.to_string_lossy() {
                let mut node = self.get_node(&a_path);
                node.add(&last_path, is_file);
                // After we add the first file, everything else will be directories.
                is_file = false;
                self.tree.insert(FileGraphNode::key(&a_path), node);
            }
            a_path.clone_into(&mut last_path);
        }
    }

    fn get_node(&self, path: &Path) -> FileGraphNode {
        match self.tree.get(&FileGraphNode::key(path)) {
            Some(n) => n.copy(),
            None => FileGraphNode::new(path),
        }
    }

    fn all_children(&self, path: &Path) -> Vec<PathBuf> {
        // All children of this directory will use keys prefixed with this
        // directory's key.
        let node_key = FileGraphNode::key(path);
        self.tree
            .keys()
            .filter(|k| k.starts_with(&node_key))
            .map(PathBuf::from)
            .collect()
    }

    fn remove_from_graph(&mut self, path: &Path) {
        // If this is a directory, remove it and its children from the graph
        let node = self.get_node(path);
        if !node.is_empty() {
            for path in self.all_children(path) {
                // delete the node and object
                self.tree.remove(&FileGraphNode::key(&path));
                self.delete_file_only(&path);
            }
        }

        let mut last_path: PathBuf = path.to_path_buf();
        for ancestor in path.ancestors() {
            let a_path = ancestor.to_path_buf();
            if a_path.to_string_lossy() == path.to_string_lossy() {
                // We've handled the leaf above
                continue;
            }
            let mut node = self.get_node(&a_path);
            node.remove(&last_path);
            if node.is_empty() {
                // If this is the last item in a node, remove the node entirely.
                self.tree.remove(&FileGraphNode::key(&a_path));
            } else {
                // If the file has siblings, just write the updated value and
                // stop traversing.
                self.tree.insert(FileGraphNode::key(&a_path), node);
                break;
            }
            a_path.clone_into(&mut last_path);
        }
    }

    fn delete_file_only(&mut self, path: &Path) {
        self.fs.remove(&path.to_string_lossy().to_lowercase());
    }

    fn delete_file(&mut self, path: &Path) {
        self.delete_file_only(path);
        self.remove_from_graph(path);
    }

    fn read_file(&self, path: &Path) -> Option<Vec<u8>> {
        debug!("read {}", path.display());
        self.fs
            .get(&path.to_string_lossy().to_lowercase())
            .map(|v| v.to_vec())
    }
}

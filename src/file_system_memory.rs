use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    error::Error,
    path::{Path, PathBuf},
};
use tracing::debug;

use crate::ArchivalError;

use super::{FileSystemAPI, WatchableFileSystemAPI};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileGraphNode {
    path: PathBuf,
    pub(crate) files: HashSet<PathBuf>,
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
    pub fn add(&mut self, path: &PathBuf) {
        self.files.insert(path.to_owned());
    }
    pub fn remove(&mut self, path: &PathBuf) {
        self.files.remove(path);
    }
    pub fn copy(&self) -> Self {
        Self {
            path: self.path.to_path_buf(),
            files: self.files.iter().map(|r| r.to_path_buf()).collect(),
        }
    }
}

pub struct Watcher {
    pub(crate) root: PathBuf,
    pub(crate) paths: Vec<String>,
    pub(crate) changed: Box<dyn Fn(Vec<PathBuf>) + Send + Sync>,
}

#[derive(Default)]
pub struct MemoryFileSystem {
    fs: HashMap<String, Vec<u8>>,
    tree: HashMap<String, FileGraphNode>,
    change_handlers: VecDeque<Watcher>,
}

impl FileSystemAPI for MemoryFileSystem {
    fn exists(&self, path: &Path) -> Result<bool, Box<dyn Error>> {
        if self
            .fs
            .get(&path.to_string_lossy().to_lowercase())
            .is_some()
            || self.is_dir(path)?
        {
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn is_dir(&self, path: &Path) -> Result<bool, Box<dyn Error>> {
        Ok(self.tree.get(&FileGraphNode::key(path)).is_some())
    }
    fn remove_dir_all(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        self.remove_from_graph(path);
        Ok(())
    }
    fn create_dir_all(&mut self, _path: &Path) -> Result<(), Box<dyn Error>> {
        // dirs are implicitly created when files are created in them
        Ok(())
    }
    fn read_dir(&self, path: &Path) -> Result<Vec<std::path::PathBuf>, Box<dyn Error>> {
        let files = self.get_files(path);
        Ok(files.iter().map(|pb| pb.to_path_buf()).collect())
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
        self.files_changed(vec![path.to_path_buf()])?;
        Ok(())
    }
    fn write(&mut self, path: &Path, contents: Vec<u8>) -> Result<(), Box<dyn Error>> {
        if self.is_dir(path)? {
            return Err(ArchivalError::new("cannot write to a folder").into());
        }
        self.write_file(path, contents);
        self.files_changed(vec![path.to_path_buf()])?;
        Ok(())
    }
    fn write_str(&mut self, path: &Path, contents: String) -> Result<(), Box<dyn Error>> {
        self.write(path, contents.as_bytes().to_vec())
    }
    fn copy_contents(&mut self, from: &Path, to: &Path) -> Result<(), Box<dyn Error>> {
        if let Some(file) = self.read_file(from) {
            self.write_file(to, file);
            self.files_changed(vec![to.to_path_buf()])?;
        }
        Ok(())
    }
    fn walk_dir(&self, path: &Path) -> Result<Box<dyn Iterator<Item = PathBuf>>, Box<dyn Error>> {
        let path = path.to_path_buf();
        let children = self.all_children(&path);
        let mut all_files: Vec<PathBuf> = vec![];
        for child in children {
            let node = self.get_node(&child);
            all_files.append(&mut node.files.into_iter().collect());
        }
        Ok(Box::new(all_files.into_iter()))
    }
}

impl WatchableFileSystemAPI for MemoryFileSystem {
    fn watch(
        &mut self,
        root: PathBuf,
        watch_paths: Vec<String>,
        changed: impl Fn(Vec<PathBuf>) + Send + Sync + 'static,
    ) -> Result<Box<dyn FnOnce() + '_>, Box<dyn Error>> {
        let watcher = Watcher {
            root,
            paths: watch_paths,
            changed: Box::new(changed),
        };
        self.change_handlers.push_back(watcher);
        let idx = self.change_handlers.len() - 1;
        Ok(Box::new(move || {
            self.change_handlers.remove(idx);
        }))
    }
}

fn copy_path_arr(arr: &[PathBuf]) -> Vec<PathBuf> {
    arr.iter().map(|r| r.to_owned()).collect()
}

impl MemoryFileSystem {
    fn files_changed(&mut self, paths: Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
        for ch in &self.change_handlers {
            for watched in &ch.paths {
                let fp = ch.root.join(watched);
                let prefix = &fp.to_string_lossy().to_lowercase();
                let changed_paths: Vec<PathBuf> = copy_path_arr(&paths)
                    .into_iter()
                    .filter(|p| p.to_string_lossy().to_lowercase().starts_with(prefix))
                    .collect();
                if !changed_paths.is_empty() {
                    (ch.changed)(changed_paths);
                }
            }
        }
        Ok(())
    }

    fn write_file(&mut self, path: &Path, data: Vec<u8>) {
        debug!("write: {}", path.display());
        self.fs.insert(path.to_string_lossy().to_lowercase(), data);
        self.write_to_graph(path);
    }

    fn write_to_graph(&mut self, path: &Path) {
        // Traverse up the path and add each file to its parent's node
        let mut last_path: PathBuf = PathBuf::new();
        for ancestor in path.ancestors() {
            let a_path = ancestor.to_path_buf();
            // Skip the actual file path, since only directories are nodes
            if a_path.to_string_lossy() != path.to_string_lossy() {
                let mut node = self.get_node(&a_path);
                node.add(&last_path);
                self.tree.insert(FileGraphNode::key(&a_path), node);
            }
            last_path = a_path.to_owned();
        }
    }

    fn get_node(&self, path: &Path) -> FileGraphNode {
        match self.tree.get(&FileGraphNode::key(path)) {
            Some(n) => n.copy(),
            None => FileGraphNode::new(path),
        }
    }

    fn get_files(&self, path: &Path) -> HashSet<PathBuf> {
        self.get_node(path).files
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
            last_path = a_path.to_owned();
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

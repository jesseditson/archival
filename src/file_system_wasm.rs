use futures::executor::block_on;
use indexed_db_futures::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    error::Error,
    future::Future,
    path::{Path, PathBuf},
};
use wasm_bindgen::JsValue;
use web_sys::{DomException, IdbKeyRange};

use crate::file_system_mutex::FileSystemMutex;

use super::{FileSystemAPI, WatchableFileSystemAPI};

static FILES_STORE_NAME: &str = "files";
static FILE_GRAPH_STORE_NAME: &str = "file_graph";

#[derive(Serialize, Deserialize)]
struct FileGraphNode {
    path: PathBuf,
    files: HashSet<PathBuf>,
}

impl FileGraphNode {
    pub fn from_val(path: &PathBuf, val: Option<JsValue>) -> Self {
        match val {
            Some(js_val) => {
                let val = match serde_wasm_bindgen::from_value::<Self>(js_val) {
                    Ok(v) => Some(v),
                    Err(_) => None,
                };
                val
            }
            None => None,
        }
        .unwrap_or_else(|| Self::new(&path))
    }
    pub fn new(path: &PathBuf) -> Self {
        Self {
            path: path.to_path_buf(),
            files: HashSet::new(),
        }
    }
    pub fn key(path: &PathBuf) -> String {
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
}

struct Watcher {
    root: PathBuf,
    paths: Vec<String>,
    changed: Box<dyn Fn(Vec<PathBuf>) + Send + Sync>,
}

pub struct WasmFileSystem {
    idb_name: String,
    version: u32,
    change_handlers: VecDeque<Watcher>,
}

impl WasmFileSystem {
    pub fn new(idb_name: &str) -> Self {
        Self {
            version: 1,
            idb_name: idb_name.to_owned(),
            change_handlers: VecDeque::new(),
        }
    }
}

#[derive(Debug)]
struct IdbError {
    message: String,
}
impl IdbError {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_owned(),
        }
    }
}
impl std::fmt::Display for IdbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {}", self.message)
    }
}
impl Error for IdbError {
    fn description(&self) -> &str {
        &self.message
    }
}

fn idb_task<T>(r: impl Future<Output = Result<T, DomException>>) -> Result<T, IdbError> {
    map_idb_err(block_on(r))
}
fn map_idb_err<T>(r: Result<T, DomException>) -> Result<T, IdbError> {
    r.map_err(|e| IdbError::new(&e.message()))
}

impl FileSystemAPI for WasmFileSystem {
    fn remove_dir_all(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        let db = idb_task(self.get_db())?;
        idb_task(self.remove_from_graph(&path.to_path_buf(), &db))?;
        Ok(())
    }
    fn create_dir_all(&self, _path: &Path) -> Result<(), Box<dyn Error>> {
        // dirs are implicitly created when files are created in them
        Ok(())
    }
    fn read_dir(&self, path: &Path) -> Result<Vec<std::path::PathBuf>, Box<dyn Error>> {
        let files = idb_task(self.get_files(&path.to_path_buf()))?;
        Ok(files.into_iter().collect())
    }
    fn read_to_string(&self, path: &Path) -> Result<Option<String>, Box<dyn Error>> {
        if let Some(file) = idb_task(self.read_file(&path.to_path_buf()))? {
            Ok(Some(std::str::from_utf8(&file)?.to_string()))
        } else {
            Ok(None)
        }
    }
    fn write(&self, path: &Path, contents: String) -> Result<(), Box<dyn Error>> {
        idb_task(self.write_file(&path.to_path_buf(), &contents.as_bytes().to_vec()))?;
        Ok(())
    }
    fn copy_contents(&self, from: &Path, to: &Path) -> Result<(), Box<dyn Error>> {
        if let Some(file) = idb_task(self.read_file(&from.to_path_buf()))? {
            idb_task(self.write_file(&to.to_path_buf(), &file))?;
        }
        Ok(())
    }
    fn walk_dir(&self, path: &Path) -> Result<Box<dyn Iterator<Item = PathBuf>>, Box<dyn Error>> {
        let db = idb_task(self.get_db())?;
        let tx = map_idb_err(
            db.transaction_on_one_with_mode(FILE_GRAPH_STORE_NAME, IdbTransactionMode::Readwrite),
        )?;
        let path = path.to_path_buf();
        let store = map_idb_err(tx.object_store(FILE_GRAPH_STORE_NAME))?;
        let children = idb_task(self.all_children(&path, &store))?;
        let mut all_files: Vec<PathBuf> = vec![];
        for child in children {
            let node_data = idb_task(map_idb_err(store.get_owned(&FileGraphNode::key(&child)))?)?;
            let node = FileGraphNode::from_val(&path, node_data);
            all_files.append(&mut node.files.into_iter().collect());
        }
        Ok(Box::new(all_files.into_iter()))
    }
}

impl WatchableFileSystemAPI for WasmFileSystem {
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

impl WasmFileSystem {
    fn file_changed(self, path: &PathBuf) -> Result<(), Box<dyn Error>> {
        let fs = FileSystemMutex::init(self);
        fs.clone().with_fs(|fs| {
            for ch in &fs.change_handlers {
                for watched in &ch.paths {
                    let fp = ch.root.join(watched);
                    if path
                        .to_string_lossy()
                        .to_lowercase()
                        .starts_with(&fp.to_string_lossy().to_lowercase())
                    {
                        (ch.changed)(vec![path.into()]);
                    }
                }
            }
            Ok(())
        })?;
        Ok(())
    }

    async fn get_db(&self) -> Result<IdbDatabase, DomException> {
        let mut db_req = IdbDatabase::open_u32(&self.idb_name, self.version)?;
        db_req.set_on_upgrade_needed(Some(|evt: &IdbVersionChangeEvent| -> Result<(), JsValue> {
            // Check if the object store exists; create it if it doesn't
            if let None = evt
                .db()
                .object_store_names()
                .find(|n| n == FILES_STORE_NAME)
            {
                evt.db().create_object_store(FILES_STORE_NAME)?;
            }
            if let None = evt
                .db()
                .object_store_names()
                .find(|n| n == FILE_GRAPH_STORE_NAME)
            {
                evt.db().create_object_store(FILE_GRAPH_STORE_NAME)?;
            }
            Ok(())
        }));
        let db = db_req.into_future().await?;
        Ok(db)
    }

    async fn write_file(&self, path: &PathBuf, data: &Vec<u8>) -> Result<(), DomException> {
        let db = self.get_db().await?;

        let tx =
            db.transaction_on_one_with_mode(FILES_STORE_NAME, IdbTransactionMode::Readwrite)?;
        let store = tx.object_store(FILES_STORE_NAME)?;
        let val = serde_wasm_bindgen::to_value(data)
            .map_err(|err| DomException::new_with_message(&err.to_string()).unwrap())?;
        store.put_key_val_owned(path.to_string_lossy().to_lowercase(), &val)?;

        // IDBTransactions can have an Error or an Abort event; into_result() turns both into a
        // DOMException
        tx.await.into_result()?;

        self.write_to_graph(path, &db).await?;
        Ok(())
    }

    async fn write_to_graph(&self, path: &PathBuf, db: &IdbDatabase) -> Result<(), DomException> {
        let tx =
            db.transaction_on_one_with_mode(FILE_GRAPH_STORE_NAME, IdbTransactionMode::Readwrite)?;
        let store = tx.object_store(FILE_GRAPH_STORE_NAME)?;

        // Traverse up the path and add each file to its parent's node
        let mut last_path: PathBuf;
        for ancestor in path.ancestors() {
            // Skip the actual file path, since only directories are nodes
            let a_path = ancestor.to_path_buf();
            last_path = a_path.to_owned();
            if a_path.to_string_lossy() == path.to_string_lossy() {
                continue;
            }
            let mut node = FileGraphNode::from_val(
                &path,
                store.get_owned(FileGraphNode::key(&a_path))?.await?,
            );
            node.add(&last_path);
            let val = serde_wasm_bindgen::to_value(&node)
                .map_err(|err| DomException::new_with_message(&err.to_string()).unwrap())?;
            store.put_key_val_owned(a_path.to_string_lossy().to_lowercase(), &val)?;
        }

        Ok(())
    }

    async fn get_files(&self, path: &PathBuf) -> Result<HashSet<PathBuf>, DomException> {
        let db = self.get_db().await?;
        let tx =
            db.transaction_on_one_with_mode(FILE_GRAPH_STORE_NAME, IdbTransactionMode::Readwrite)?;
        let store = tx.object_store(FILE_GRAPH_STORE_NAME)?;
        let node =
            FileGraphNode::from_val(&path, store.get_owned(&FileGraphNode::key(&path))?.await?);
        Ok(node.files)
    }

    async fn all_children(
        &self,
        path: &PathBuf,
        store: &IdbObjectStore<'_>,
    ) -> Result<Vec<PathBuf>, DomException> {
        // All children of this directory will use keys prefixed with this
        // directory's key. This means we can use a bound with the highest
        // possible value to get all the keys
        let node_key = FileGraphNode::key(&path);
        let range_key = IdbKeyRange::bound(
            &JsValue::from_str(&node_key),
            &JsValue::from_str(&format!("{}\u{ffff}", node_key)),
        )
        .map_err(|jv| DomException::from(jv))?;
        let all_keys = store.get_all_with_key(&range_key)?.await?;
        Ok(all_keys
            .iter()
            .map(|key| PathBuf::from(key.as_string().unwrap()))
            .collect())
    }

    async fn remove_from_graph<'a>(
        &self,
        path: &PathBuf,
        db: &IdbDatabase,
    ) -> Result<(), DomException> {
        let tx =
            db.transaction_on_one_with_mode(FILE_GRAPH_STORE_NAME, IdbTransactionMode::Readwrite)?;
        let store = tx.object_store(FILE_GRAPH_STORE_NAME)?;

        // If this is a directory, remove it and its children from the graph
        let node =
            FileGraphNode::from_val(&path, store.get_owned(&FileGraphNode::key(&path))?.await?);
        if !node.is_empty() {
            for path in self.all_children(&path, &store).await? {
                // delete the node and object
                store.delete_owned(path.to_str())?;
                self.delete_file_only(&path, &db).await?;
            }
        }

        let mut last_path: PathBuf;
        for ancestor in path.ancestors() {
            let a_path = ancestor.to_path_buf();
            last_path = a_path.to_owned();
            if a_path.to_string_lossy() == path.to_string_lossy() {
                // We've handled the leaf above
                continue;
            }
            let mut node = FileGraphNode::from_val(
                &a_path,
                store.get_owned(FileGraphNode::key(&a_path))?.await?,
            );
            node.remove(&last_path);
            if node.is_empty() {
                // If this is the last item in a node, remove the node entirely.
                store.delete_owned(FileGraphNode::key(&a_path))?;
            } else {
                // If the file has siblings, just write the updated value and
                // stop traversing.
                let val = serde_wasm_bindgen::to_value(&node)
                    .map_err(|err| DomException::new_with_message(&err.to_string()).unwrap())?;
                store.put_key_val_owned(a_path.to_string_lossy().to_lowercase(), &val)?;
                break;
            }
        }
        tx.await.into_result()?;
        Ok(())
    }

    async fn delete_file_only(&self, path: &PathBuf, db: &IdbDatabase) -> Result<(), DomException> {
        let tx =
            db.transaction_on_one_with_mode(FILES_STORE_NAME, IdbTransactionMode::Readwrite)?;
        let store = tx.object_store(FILES_STORE_NAME)?;
        store.delete_owned(path.to_string_lossy().to_lowercase())?;
        tx.await.into_result()?;
        Ok(())
    }

    async fn delete_file(&self, path: &PathBuf) -> Result<(), DomException> {
        let db = self.get_db().await?;
        self.delete_file_only(path, &db).await?;
        self.remove_from_graph(path, &db).await?;
        Ok(())
    }

    async fn read_file(&self, path: &PathBuf) -> Result<Option<Vec<u8>>, DomException> {
        let db = self.get_db().await?;

        let tx = db.transaction_on_one(FILES_STORE_NAME)?;
        let store = tx.object_store(FILES_STORE_NAME)?;

        match store
            .get_owned(path.to_string_lossy().to_lowercase())?
            .await?
        {
            Some(js_val) => {
                let val = match serde_wasm_bindgen::from_value::<Vec<u8>>(js_val) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        println!("INVALID VALUE FOR {}: {}", path.display(), e);
                        None
                    }
                };
                Ok(val)
            }
            None => Ok(None),
        }
    }
}

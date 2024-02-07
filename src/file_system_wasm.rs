use crate::{file_system::FileSystemAPI, file_system_memory::FileGraphNode, ArchivalError};
use indexed_db_futures::prelude::*;
use std::{
    collections::HashSet,
    error::Error,
    future::Future,
    path::{Path, PathBuf},
};
use tracing::debug;
use wasm_bindgen::JsValue;
use web_sys::{DomException, IdbKeyRange};

static FILES_STORE_NAME: &str = "files";
static FILE_GRAPH_STORE_NAME: &str = "file_graph";

pub struct WasmFileSystem {
    idb_name: String,
    version: u32,
}

impl WasmFileSystem {
    pub fn new(idb_name: &str) -> Self {
        debug!("init wasm filesystem");
        Self {
            version: 1,
            idb_name: idb_name.to_owned(),
        }
    }
}

impl FileGraphNode {
    pub fn from_js_val(path: &Path, val: Option<JsValue>) -> Self {
        match val {
            Some(js_val) => match serde_wasm_bindgen::from_value::<Self>(js_val) {
                Ok(v) => Some(v),
                Err(_) => None,
            },
            None => None,
        }
        .unwrap_or_else(|| Self::new(path))
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

fn idb_task<T>(_r: impl Future<Output = Result<T, DomException>>) -> Result<T, IdbError> {
    todo!("figure out if we can run this future inline or use a different approach");
    // map_idb_err(block_on(r))
}
fn map_idb_err<T>(r: Result<T, DomException>) -> Result<T, IdbError> {
    r.map_err(|e| IdbError::new(&e.message()))
}

impl FileSystemAPI for WasmFileSystem {
    fn exists(&self, path: &Path) -> Result<bool, Box<dyn Error>> {
        debug!("exists {}", path.display());
        if idb_task(self.get_file(path))?.is_some() || self.is_dir(path)? {
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn is_dir(&self, path: &Path) -> Result<bool, Box<dyn Error>> {
        let node_data = idb_task(async {
            let db = self.get_db().await?;
            let tx = db.transaction_on_one_with_mode(
                FILE_GRAPH_STORE_NAME,
                IdbTransactionMode::Readwrite,
            )?;
            let store = tx.object_store(FILE_GRAPH_STORE_NAME)?;
            store.get_owned(&FileGraphNode::key(path))?.await
        })?;
        Ok(node_data.is_some())
    }
    fn remove_dir_all(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        let db = idb_task(self.get_db())?;
        idb_task(self.remove_from_graph(path, &db))?;
        Ok(())
    }
    fn create_dir_all(&mut self, _path: &Path) -> Result<(), Box<dyn Error>> {
        // dirs are implicitly created when files are created in them
        Ok(())
    }
    fn read_dir(&self, path: &Path) -> Result<Vec<std::path::PathBuf>, Box<dyn Error>> {
        let files = idb_task(self.get_files(path))?;
        Ok(files.into_iter().collect())
    }
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        Ok(idb_task(self.read_file(path))?)
    }
    fn read_to_string(&self, path: &Path) -> Result<Option<String>, Box<dyn Error>> {
        if let Some(file) = idb_task(self.read_file(path))? {
            Ok(Some(std::str::from_utf8(&file)?.to_string()))
        } else {
            Ok(None)
        }
    }
    fn delete(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        if self.is_dir(path)? {
            return Err(ArchivalError::new("use remove_dir_all to delete directories").into());
        }
        idb_task(self.delete_file(path))?;
        Ok(())
    }
    fn write(&mut self, path: &Path, contents: Vec<u8>) -> Result<(), Box<dyn Error>> {
        if self.is_dir(path)? {
            return Err(ArchivalError::new("cannot write to a folder").into());
        }
        debug!("write: {}", path.display());
        idb_task(self.write_file(path, &contents))?;
        Ok(())
    }
    fn write_str(&mut self, path: &Path, contents: String) -> Result<(), Box<dyn Error>> {
        self.write(path, contents.as_bytes().to_vec())
    }
    fn copy_recursive(&mut self, from: &Path, to: &Path) -> Result<(), Box<dyn Error>> {
        debug!("copy {} -> {}", from.display(), to.display());
        let mut changed_paths = vec![];
        if !self.is_dir(from)? {
            if let Some(file) = idb_task(self.read_file(from))? {
                idb_task(self.write_file(to, &file))?;
                changed_paths.push(to.to_path_buf());
            }
        } else {
            for child in self.walk_dir(from)? {
                let dest = to.join(child.strip_prefix(from)?);
                debug!("copy {} -> {}", child.display(), dest.display());
                if let Some(file) = idb_task(self.read_file(&child))? {
                    idb_task(self.write_file(&dest, &file))?;
                    changed_paths.push(to.to_path_buf());
                }
            }
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
            let node = FileGraphNode::from_js_val(&path, node_data);
            all_files.append(&mut node.files.into_iter().collect());
        }
        Ok(Box::new(all_files.into_iter()))
    }
}

impl WasmFileSystem {
    async fn get_db(&self) -> Result<IdbDatabase, DomException> {
        let mut db_req = IdbDatabase::open_u32(&self.idb_name, self.version)?;
        db_req.set_on_upgrade_needed(Some(|evt: &IdbVersionChangeEvent| -> Result<(), JsValue> {
            // Check if the object store exists; create it if it doesn't
            if !evt.db().object_store_names().any(|n| n == FILES_STORE_NAME) {
                evt.db().create_object_store(FILES_STORE_NAME)?;
            }
            if !evt
                .db()
                .object_store_names()
                .any(|n| n == FILE_GRAPH_STORE_NAME)
            {
                evt.db().create_object_store(FILE_GRAPH_STORE_NAME)?;
            }
            Ok(())
        }));
        let db = db_req.into_future().await?;
        Ok(db)
    }

    async fn write_file(&self, path: &Path, data: &Vec<u8>) -> Result<(), DomException> {
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

    async fn write_to_graph(&self, path: &Path, db: &IdbDatabase) -> Result<(), DomException> {
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
            let mut node = FileGraphNode::from_js_val(
                &a_path,
                store.get_owned(FileGraphNode::key(&a_path))?.await?,
            );
            node.add(&last_path);
            let val = serde_wasm_bindgen::to_value(&node)
                .map_err(|err| DomException::new_with_message(&err.to_string()).unwrap())?;
            store.put_key_val_owned(a_path.to_string_lossy().to_lowercase(), &val)?;
        }

        Ok(())
    }

    async fn get_files(&self, path: &Path) -> Result<HashSet<PathBuf>, DomException> {
        let db = self.get_db().await?;
        let tx =
            db.transaction_on_one_with_mode(FILE_GRAPH_STORE_NAME, IdbTransactionMode::Readwrite)?;
        let store = tx.object_store(FILE_GRAPH_STORE_NAME)?;
        let node =
            FileGraphNode::from_js_val(path, store.get_owned(&FileGraphNode::key(path))?.await?);
        Ok(node.files)
    }

    async fn all_children(
        &self,
        path: &Path,
        store: &IdbObjectStore<'_>,
    ) -> Result<Vec<PathBuf>, DomException> {
        // All children of this directory will use keys prefixed with this
        // directory's key. This means we can use a bound with the highest
        // possible value to get all the keys
        let node_key = FileGraphNode::key(path);
        let range_key = IdbKeyRange::bound(
            &JsValue::from_str(&node_key),
            &JsValue::from_str(&format!("{}\u{ffff}", node_key)),
        )
        .map_err(DomException::from)?;
        let all_keys = store.get_all_with_key(&range_key)?.await?;
        Ok(all_keys
            .iter()
            .map(|key| PathBuf::from(key.as_string().unwrap()))
            .collect())
    }

    async fn remove_from_graph<'a>(
        &self,
        path: &Path,
        db: &IdbDatabase,
    ) -> Result<(), DomException> {
        let tx =
            db.transaction_on_one_with_mode(FILE_GRAPH_STORE_NAME, IdbTransactionMode::Readwrite)?;
        let store = tx.object_store(FILE_GRAPH_STORE_NAME)?;
        // If this is a directory, remove it and its children from the graph
        let node =
            FileGraphNode::from_js_val(path, store.get_owned(&FileGraphNode::key(path))?.await?);
        if !node.is_empty() {
            for path in self.all_children(path, &store).await? {
                // delete the node and object
                store.delete_owned(path.to_str())?;
                self.delete_file_only(&path, db).await?;
            }
        }

        let mut last_path: PathBuf = path.to_path_buf();
        for ancestor in path.ancestors() {
            let a_path = ancestor.to_path_buf();
            if a_path.to_string_lossy() == path.to_string_lossy() {
                // We've handled the leaf above
                continue;
            }
            let mut node = FileGraphNode::from_js_val(
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
            last_path = a_path.to_owned();
        }
        tx.await.into_result()?;
        Ok(())
    }

    async fn delete_file_only(&self, path: &Path, db: &IdbDatabase) -> Result<(), DomException> {
        let tx =
            db.transaction_on_one_with_mode(FILES_STORE_NAME, IdbTransactionMode::Readwrite)?;
        let store = tx.object_store(FILES_STORE_NAME)?;
        store.delete_owned(path.to_string_lossy().to_lowercase())?;
        tx.await.into_result()?;
        Ok(())
    }

    async fn delete_file(&self, path: &Path) -> Result<(), DomException> {
        let db = self.get_db().await?;
        self.delete_file_only(path, &db).await?;
        self.remove_from_graph(path, &db).await?;
        Ok(())
    }

    async fn get_file(&self, path: &Path) -> Result<Option<JsValue>, DomException> {
        let db = self.get_db().await?;

        let tx = db.transaction_on_one(FILES_STORE_NAME)?;
        let store = tx.object_store(FILES_STORE_NAME)?;

        store
            .get_owned(path.to_string_lossy().to_lowercase())?
            .await
    }

    async fn read_file(&self, path: &Path) -> Result<Option<Vec<u8>>, DomException> {
        match self.get_file(path).await? {
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

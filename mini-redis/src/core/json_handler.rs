use std::{collections::HashMap, io::ErrorKind, path::Path, sync::Arc};

use anyhow::Context;
use log::debug;
use serde_json::Value;
use tokio::{
    fs::{self, File},
    io::{AsyncBufReadExt, AsyncSeekExt, AsyncWriteExt, BufReader},
    sync::RwLock,
};

/// Handles JSON files. Allows reading, writing, deleting keys.
pub struct JsonFileHandler {
    data: Arc<RwLock<HashMap<String, Value>>>,
    file: File,
}

// Create
impl JsonFileHandler {
    pub async fn from_path(path: impl AsRef<Path>) -> anyhow::Result<JsonFileHandler> {
        let path = path.as_ref();
        let parent_dir = path.parent().context(format!(
            "could not read parent directory of path {:?}",
            path
        ))?;

        if let Err(e) = fs::create_dir_all(parent_dir).await {
            match e.kind() {
                ErrorKind::AlreadyExists => {}
                _ => return Err(anyhow::Error::context(e.into(), "Failed")),
            }
        }

        // Keep file open until shutdown
        let config_file = fs::File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .read(true)
            .open(path)
            .await
            .context(format!("could not open file at {:?}", path))?;

        let mut reader = BufReader::new(config_file);
        reader.fill_buf().await.unwrap();

        debug!("Parsing config");
        let data: HashMap<String, Value> =
            serde_json::from_reader(reader.buffer()).unwrap_or_default();

        let file = reader.into_inner();

        Ok(Self {
            file,
            data: Arc::new(RwLock::new(data)),
        })
    }
}

// Operations
impl JsonFileHandler {
    pub async fn write(&mut self, key: &str, value: Value) -> Option<Value> {
        self.data.write().await.insert(key.to_string(), value)
    }

    pub async fn read(&mut self, key: &str) -> Option<Value> {
        self.data.read().await.get(key).cloned()
    }

    pub async fn delete(&mut self, key: &str) -> Option<Value> {
        self.data.write().await.remove(key)
    }

    pub async fn sync(&mut self) {
        let data = serde_json::to_vec_pretty(&(*self.data.read().await)).unwrap();
        self.file.set_len(0).await.unwrap();
        self.file.rewind().await.unwrap();
        self.file.write_all(&data).await.unwrap();
        self.file.flush().await.unwrap();
    }

    pub async fn close(&mut self) {
        self.file.flush().await.unwrap();
        self.file.shutdown().await.unwrap();
    }
}

use std::{collections::HashMap, io::ErrorKind, path::Path};

use anyhow::Context;
use log::debug;
use serde_json::Value;
use tokio::{
    fs::{self, File},
    io::{AsyncBufReadExt, AsyncSeekExt, AsyncWriteExt, BufReader},
    sync::{Mutex, RwLock},
};

/// Handles JSON files. Allows reading, writing, deleting keys.
pub struct JsonFileHandler {
    data: RwLock<HashMap<String, Value>>,
    file: Mutex<File>,
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
            file: Mutex::new(file),
            data: RwLock::new(data),
        })
    }
}

// Operations
impl JsonFileHandler {
    pub async fn write(&self, key: &str, value: Value) -> Option<Value> {
        self.data.write().await.insert(key.to_string(), value)
    }

    pub async fn read(&self, key: &str) -> Option<Value> {
        self.data.read().await.get(key).cloned()
    }

    pub async fn delete(&self, key: &str) -> Option<Value> {
        self.data.write().await.remove(key)
    }

    pub async fn sync(&self) {
        let mut file = self.file.lock().await;
        let data = self.data.read().await.clone();
        let bytes = serde_json::to_vec_pretty(&data).unwrap();

        file.set_len(0).await.unwrap();
        file.rewind().await.unwrap();
        file.write_all(&bytes).await.unwrap();
        file.flush().await.unwrap();
    }

    pub async fn close(&self) {
        let mut file = self.file.lock().await;

        file.flush().await.unwrap();
        file.shutdown().await.unwrap();
    }
}

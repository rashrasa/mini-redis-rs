use std::{collections::HashMap, sync::Arc};

use log::info;
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
    pub async fn from_path(path: &str) -> Result<JsonFileHandler, tokio::io::Error> {
        let path = std::path::Path::new(path);
        fs::create_dir_all(path.parent().unwrap()).await.unwrap();

        // Keep file open until shutdown
        let config_file = fs::File::options()
            .create(true)
            .write(true)
            .read(true)
            .share_mode(0)
            .open(path)
            .await
            .unwrap();

        let mut reader = BufReader::new(config_file);
        reader.fill_buf().await.unwrap();

        info!("Parsing config");
        let data: HashMap<String, Value> =
            serde_json::from_reader(reader.buffer()).unwrap_or_default();

        let file = reader.into_inner();

        Ok(Self {
            file: file,
            data: Arc::new(RwLock::new(data)),
        })
    }
}

// Operations
impl JsonFileHandler {
    pub async fn write(&mut self, key: &str, value: Value) -> Option<Value> {
        let result = (*self.data.write().await).insert(key.to_string(), value); // lock dropped
        self.sync().await;
        result
    }

    pub async fn read(&mut self, key: &str) -> Option<Value> {
        (*self.data.read().await).get(key).cloned()
    }

    pub async fn delete(&mut self, key: &str) -> Option<Value> {
        let result = (*self.data.write().await).remove(key); // lock dropped
        self.sync().await;
        result
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

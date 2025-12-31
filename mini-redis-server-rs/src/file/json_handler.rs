use std::{marker::PhantomData, sync::Arc};

use log::info;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{
    fs::{self, File},
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    sync::Mutex,
};

/// Handles JSON files.
///
/// Watches file for changes and updates state live.
pub struct JsonFileHandler<T: Default + DeserializeOwned + Serialize> {
    data: Arc<Mutex<T>>,
    file: File,
}

// Create
impl<T: DeserializeOwned + Serialize + Default> JsonFileHandler<T> {
    pub async fn from_path(path: &str) -> Result<JsonFileHandler<T>, tokio::io::Error> {
        let path = std::path::Path::new(path);
        fs::create_dir_all(path.parent().unwrap()).await.unwrap();

        // Keep file open until shutdown
        let config_file = fs::File::options()
            .create(true)
            .write(true)
            .read(true)
            .open(path)
            .await
            .unwrap();

        let mut reader = BufReader::new(config_file);
        reader.fill_buf().await.unwrap();

        info!("Parsing config");
        let data: T = serde_json::from_reader(reader.buffer()).unwrap_or_default();

        info!("Writing to config");
        let file = reader.into_inner();

        Ok(Self {
            file: file,
            data: Arc::new(Mutex::new(data)),
        })
    }
}

impl<T: DeserializeOwned + Serialize + Default> JsonFileHandler<T> {
    pub async fn synchronized<F: for<'a> FnOnce(&'a mut T) -> S, S: Future<Output = ()>>(
        &mut self,
        callback: F,
    ) {
        todo!("Implement way to access value temporarily");
        let mut lock = self.data.lock().await;
        callback(&mut lock).await;
        return;
    }
}

impl<T: DeserializeOwned + Serialize + Default> Drop for JsonFileHandler<T> {
    fn drop(&mut self) {
        todo!("Implement Async Drop somehow for graceful shutdown");
        // let buf = serde_json::to_vec_pretty(&config).unwrap();
        // writer.write_all(&buf).await.unwrap();
        // writer.flush().await.unwrap();
    }
}

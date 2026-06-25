use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod core;
pub mod server;

pub use server::{serve, serve_from_file};

use crate::core::JsonFileHandler;

#[derive(Serialize, Deserialize, Debug)]
pub struct InsertRequest {
    pub key: String,
    pub value: Value,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReadRequest {
    pub key: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteRequest {
    pub key: String,
}

pub struct State {
    pub data: JsonFileHandler,
}

impl State {
    pub async fn write(&self, key: &str, value: Value) -> Option<Value> {
        self.data.write(key, value).await
    }

    pub async fn read(&self, key: &str) -> Option<Value> {
        self.data.read(key).await
    }

    pub async fn delete(&self, key: &str) -> Option<Value> {
        self.data.delete(key).await
    }

    pub async fn sync(&self) {
        self.data.sync().await;
    }
}

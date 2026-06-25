use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod core;
pub mod server;

pub use server::{serve, serve_from_file};

use crate::core::JsonFileHandler;

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum Request {
    Insert(String, Value),
    Delete(String),
    Read(String),
}

pub struct State {
    pub data: JsonFileHandler,
}

impl State {
    pub async fn write(&mut self, key: &str, value: Value) -> Option<Value> {
        self.data.write(key, value).await
    }

    pub async fn read(&mut self, key: &str) -> Option<Value> {
        self.data.read(key).await
    }

    pub async fn delete(&mut self, key: &str) -> Option<Value> {
        self.data.delete(key).await
    }

    pub async fn sync(&mut self) {
        self.data.sync().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_request_eq() {
        assert_eq!(Request::Read("test".into()), Request::Read("test".into()));
    }
}

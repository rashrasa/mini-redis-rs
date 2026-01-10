use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::json_handler::JsonFileHandler;

pub mod connection;
pub mod file;

const TCP_STREAM_MAX_FAILED_READS: usize = 5;

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum Request {
    Insert(String, Value),
    Delete(String),
    Read(String),
}

pub struct ServerState {
    pub data: JsonFileHandler,
}

impl ServerState {
    pub async fn write(&mut self, key: &str, value: Value) -> Option<Value> {
        self.data.write(key, value).await
    }

    pub async fn read(&mut self, key: &str) -> Option<Value> {
        self.data.read(key).await
    }

    pub async fn delete(&mut self, key: &str) -> Option<Value> {
        self.data.delete(key).await
    }
}

#[derive(Debug)]
pub enum Error {
    CrateError(Box<dyn std::error::Error + Send + Sync>),
    StdIoError(std::io::Error),
    SerdeJsonError(serde_json::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CrateError(error) => write!(f, "Error: {}", error),
            Error::StdIoError(error) => write!(f, "StdIoError: {}", error),
            Error::SerdeJsonError(error) => write!(f, "SerdeJsonError: {}", error),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::StdIoError(value)
    }
}
impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::SerdeJsonError(value)
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for Error {
    fn from(value: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Error::CrateError(value)
    }
}

mod tests {
    use super::*;

    #[test]
    fn simple_request_eq() {
        assert_eq!(Request::Read("test".into()), Request::Read("test".into()));
    }
}

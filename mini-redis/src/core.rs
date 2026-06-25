pub mod connection;
pub mod json_handler;

use std::fmt::Display;

pub use json_handler::JsonFileHandler;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Configuration {
    pub data_path: String,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            data_path: String::from("data.json"),
        }
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

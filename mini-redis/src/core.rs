pub mod connection;
pub mod file;

pub use file::JsonFileHandler;
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

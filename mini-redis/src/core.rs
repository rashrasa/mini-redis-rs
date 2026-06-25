pub mod json_handler;

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

use std::{error::Error, marker::PhantomData, sync::Arc};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::sync::RwLock;

pub mod file;

#[derive(Deserialize, Serialize)]
struct Configuration {
    data_path_abs: String,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            data_path_abs: "./data/data.json".to_string(),
        }
    }
}

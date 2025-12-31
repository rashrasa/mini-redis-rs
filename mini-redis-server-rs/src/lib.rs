use serde::{Deserialize, Serialize};

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

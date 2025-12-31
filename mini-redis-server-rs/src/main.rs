use std::path;

use log::info;
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
};

use serde::{Deserialize, Serialize};
const CONFIG_PATH_STR: &str = "./data/config.json";

#[tokio::main]
async fn main() {
    let config_path = path::Path::new(CONFIG_PATH_STR);
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .target(env_logger::Target::Stdout)
        .init();

    info!("Reading config");

    tokio::time::sleep(tokio::time::Duration::new(10, 0)).await;
}

// HIGH LEVEL
// 1. Read config file and initialize config struct
// 2. Lock database file (json file) and keep it open during entire run
// 3. Accept socket connections and spawn a task for each and close appropriately
// 4. Listen for insert, read, delete requests
// 5. Perform operations atomically

use std::{path, time::Duration};

use log::info;
use mini_redis_server_rs::file::json_handler;
use rand::Rng;
use serde_json::{Number, json};
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    select,
    task::JoinSet,
};

use serde::{Deserialize, Serialize};
const CONFIG_PATH_STR: &str = "./data/config.json";

#[tokio::main]
async fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .target(env_logger::Target::Stdout)
        .init();

    info!("Reading config");
    let mut config = json_handler::JsonFileHandler::from_path(CONFIG_PATH_STR)
        .await
        .unwrap();

    let data_path = match config.read("data_path_abs").await {
        Some(path) => match path {
            serde_json::Value::String(s) => s,
            _ => "data/data.json".into(),
        },
        None => "data/data.json".into(),
    };

    let mut data = json_handler::JsonFileHandler::from_path(&data_path)
        .await
        .unwrap();

    data.write("test", serde_json::Value::String("test value".into()))
        .await;

    data.write(
        "rand_number_1",
        serde_json::Value::Number(Number::from_f64(rand::rng().random()).unwrap()),
    )
    .await;

    data.write(
        "rand_number_2",
        serde_json::Value::Number(Number::from_f64(rand::rng().random()).unwrap()),
    )
    .await;

    info!("Completed startup");

    tokio::spawn(async move {
        // Start application loop
    });
    // Wait to terminate
    tokio::signal::ctrl_c().await.unwrap();
    info!("Starting graceful shutdown");
    let mut js = JoinSet::new();
    js.spawn(async move {
        data.close().await;
    });
    js.join_all().await;
}

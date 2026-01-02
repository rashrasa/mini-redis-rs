// HIGH LEVEL
// 1. Read config file and initialize config struct
// 2. Lock database file (json file) and keep it open during entire run
// 3. Accept socket connections and spawn a task for each and close appropriately
// 4. Listen for insert, read, delete requests
// 5. Perform operations atomically

use std::{path, sync::Arc, time::Duration};

use log::info;
use mini_redis_server_rs::file::json_handler;
use rand::Rng;
use serde_json::{Number, json};
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufStream, BufWriter},
    net::TcpListener,
    select,
    sync::Mutex,
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

    // TODO: Find way to cancel accept() and read()

    // Accept new connections
    tokio::spawn(async move {
        let listener: TcpListener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
        info!("Completed startup");
        loop {
            let (conn_stream, conn_addr) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                // Handle each connection
                let (conn_stream, conn_addr) = (conn_stream, conn_addr);
                let mut buf_stream = BufReader::new(conn_stream);
                info!("Received new connection: {}", conn_addr);
                loop {
                    let mut msg: String = "".into();
                    buf_stream.read_to_string(&mut msg).await.unwrap();
                    buf_stream.flush().await.unwrap();
                    info!("Received tcp message:\n{}", msg);
                }
            });
        }
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

// HIGH LEVEL
// 1. Read config file and initialize config struct
// 2. Lock database file (json file) and keep it open during entire run
// 3. Accept socket connections and spawn a task for each and close appropriately
// 4. Listen for insert, read, delete requests
// 5. Perform operations atomically

use log::info;
use mini_redis_server_rs::{Request, file::json_handler, server::ServerHandler};

use serde_json::{Number, Value};
use tokio::task::JoinSet;

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
    info!(
        "{}",
        serde_json::to_string(&Request::Insert(
            "Key_a".into(),
            Value::Number(Number::from_f64(3.2).unwrap())
        ))
        .unwrap()
    );
    let mut server = ServerHandler::spawn(&data_path).await.unwrap();

    // Wait to terminate
    tokio::signal::ctrl_c().await.unwrap();
    info!("Starting graceful shutdown");

    let mut js = JoinSet::new();
    js.spawn(async move {
        server.shutdown().await;
    });
    js.spawn(async move {
        config.close().await;
    });
    js.join_all().await;
}

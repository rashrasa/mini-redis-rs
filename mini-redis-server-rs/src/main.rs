// HIGH LEVEL
// 1. Read config file and initialize config struct
// 2. Lock database file (json file) and keep it open during entire run
// 3. Accept socket connections and spawn a task for each and close appropriately
// 4. Listen for insert, read, delete requests
// 5. Perform operations atomically

use std::{sync::Arc, time::Duration};

use log::{info, warn};
use mini_redis_server_rs::{
    Request, ServerState,
    connection::ConnectionHandler,
    file::json_handler::{self, JsonFileHandler},
};
use tokio::select;

use serde_json::{Number, Value};
use tokio::{net::TcpListener, sync::Mutex, task::JoinSet};

const CONFIG_PATH_STR: &str = "./data/config.json";

// TODO: Accept command-line arguments for testing (timer instead of ctrl-c to close), server config, etc.
// TODO: memory -> file sync policy

#[tokio::main]
async fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .target(env_logger::Target::Stdout)
        .init();

    warn!("Initializing tokio-console");
    console_subscriber::init();

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
    let cancellation_token = tokio_util::sync::CancellationToken::new();

    let data = JsonFileHandler::from_path(&data_path).await.unwrap();
    let state = Arc::new(Mutex::new(ServerState { data }));
    let tcp_listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

    let cancellation_token_task = cancellation_token.clone();

    let state_task = state.clone();
    let _ = tokio::spawn(async move {
        let state = state_task;
        let cancellation_token = cancellation_token_task;

        info!("Completed startup");
        loop {
            select! {
                // Accept new connections
                result = tcp_listener.accept() => {
                    let (tcp_stream, tcp_addr) = result.unwrap();
                    ConnectionHandler::spawn(tcp_stream, tcp_addr, state.clone(), &cancellation_token).await;
                }
                _ = cancellation_token.cancelled() => {
                    break;
                }
            };
        }
    });

    // Wait to terminate
    tokio::time::sleep(Duration::new(30, 0)).await;
    //tokio::signal::ctrl_c().await.unwrap();
    info!("Starting graceful shutdown");

    cancellation_token.cancel();

    state.lock().await.sync().await;

    let mut js = JoinSet::new();

    js.spawn(async move {
        config.close().await;
    });
    js.join_all().await;
}

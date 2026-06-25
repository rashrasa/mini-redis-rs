use std::sync::Arc;

use anyhow::Context;
use log::{debug, error, info};
use tokio::{net::TcpListener, select, sync::Mutex, task::JoinSet};
use tokio_util::sync::CancellationToken;

use crate::{
    State,
    core::{connection::ConnectionHandler, file::json_handler::JsonFileHandler},
};

pub async fn serve() {}

/// Serve connections using data from [path].
///
/// # Cancellation Safety
pub async fn serve_from_file(
    path: &str,
    host: &str,
    cancellation_token: CancellationToken,
) -> anyhow::Result<()> {
    let mut config = JsonFileHandler::from_path(path)
        .await
        .context(format!("failed to config file from {}", path))?;
    let data_path = match config.read("data_path_abs").await {
        Some(path) => match path {
            serde_json::Value::String(s) => s,
            _ => "data/data.json".into(),
        },
        None => "data/data.json".into(),
    };

    let data = JsonFileHandler::from_path(&data_path)
        .await
        .context(format!("failed to read data file from {}", data_path))?;
    let state = Arc::new(Mutex::new(State { data }));
    let tcp_listener = TcpListener::bind(host)
        .await
        .context(format!("failed to bind to {}", host))?;
    let state_task = state.clone();
    let state = state_task;
    info!("listening @ {}", host);

    loop {
        select! {
            result = tcp_listener.accept() => {
                let (tcp_stream, tcp_addr) = match result {
                Ok(v) => v,
                Err(e) => {
                    error!("{}", anyhow::Error::from(e).context("connection failed"));
                    continue;
                }
                };
                ConnectionHandler::spawn(tcp_stream, tcp_addr, state.clone()).await;
            }
            _ = cancellation_token.cancelled() => {
                debug!("server closing");
                break;
            }
        }
    }

    state.lock().await.sync().await;

    let mut js = JoinSet::new();

    js.spawn(async move {
        config.close().await;
    });
    js.join_all().await;

    Ok(())
}

use std::{error::Error, sync::Arc};

use futures::{StreamExt, stream::FuturesUnordered};
use log::info;
use serde_json::Value;
use tokio::{net::TcpListener, select, sync::Mutex};
use tokio_util::{future::FutureExt, sync::CancellationToken};

use crate::{Request, connection::ConnectionHandler, file::json_handler::JsonFileHandler};

pub struct ServerState {
    data: JsonFileHandler,
}

impl ServerState {
    pub async fn write(&mut self, key: &str, value: Value) -> Option<Value> {
        self.data.write(key, value).await
    }

    pub async fn read(&mut self, key: &str) -> Option<Value> {
        self.data.read(key).await
    }

    pub async fn delete(&mut self, key: &str) -> Option<Value> {
        self.data.delete(key).await
    }
}

pub struct ServerHandler {
    cancellation_token: CancellationToken,
}

impl ServerHandler {
    pub async fn spawn(data_path: &str) -> Result<Self, Arc<dyn Error>> {
        let cancellation_token = tokio_util::sync::CancellationToken::new();

        let data = JsonFileHandler::from_path(data_path).await.unwrap();
        let state = Arc::new(Mutex::new(ServerState { data }));
        let tcp_listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();

        let cancellation_token_task = cancellation_token.clone();

        let _ = tokio::spawn(async move {
            let cancellation_token = cancellation_token_task;

            info!("Completed startup");
            loop {
                // Accept new connections
                let (tcp_stream, tcp_addr) = tcp_listener.accept().await.unwrap();

                ConnectionHandler::spawn(tcp_stream, tcp_addr, state.clone(), &cancellation_token)
                    .await;
            }
        })
        .with_cancellation_token(&cancellation_token);
        Ok(Self {
            cancellation_token: cancellation_token,
        })
    }

    pub async fn shutdown(&mut self) {
        self.cancellation_token.cancel();
    }
}

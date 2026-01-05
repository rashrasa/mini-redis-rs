use std::{error::Error, net::SocketAddr, sync::Arc};

use log::info;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    select,
    sync::Mutex,
};
use tokio_util::{future::FutureExt, sync::CancellationToken};

use crate::connection::Connection;

pub struct Server {
    tcp_listener: Arc<Mutex<TcpListener>>,
    connections: Arc<Mutex<Vec<Arc<Mutex<Connection>>>>>,
    cancellation_token: CancellationToken,
}

impl Server {
    /// Spawns a tokio task which starts a server and returns a Server struct which can be used to manage it.
    pub async fn spawn() -> Result<Self, Arc<dyn Error>> {
        let cancellation_token = tokio_util::sync::CancellationToken::new();
        let tcp_listener = Arc::new(Mutex::new(
            TcpListener::bind("127.0.0.1:3000").await.unwrap(),
        ));
        let connections = Arc::new(Mutex::new(vec![]));

        let cancellation_token_task = cancellation_token.clone();
        let tcp_listener_task = tcp_listener.clone();
        let connections_task = connections.clone();
        let _ = tokio::spawn(async move {
            let listener = tcp_listener_task;
            let connections = connections_task;
            let cancellation_token = cancellation_token_task;
            info!("Completed startup");
            loop {
                // Accept new connections
                let (tcp_stream, tcp_addr) = listener.lock().await.accept().await.unwrap();

                connections.lock().await.push(Arc::new(Mutex::new(
                    Connection::spawn(tcp_stream, tcp_addr, &cancellation_token).await,
                )));
            }
        })
        .with_cancellation_token(&cancellation_token);
        Ok(Self {
            tcp_listener,
            connections,
            cancellation_token,
        })
    }

    pub fn shutdown(&mut self) {
        self.cancellation_token.cancel();
    }
}

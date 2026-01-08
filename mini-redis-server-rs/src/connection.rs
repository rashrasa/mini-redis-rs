use std::{error::Error, net::SocketAddr, sync::Arc};

use log::{error, info};
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Mutex,
};
use tokio_util::{future::FutureExt, sync::CancellationToken};

use crate::{Request, server::ServerState};

pub struct TcpStreamHandler {
    source: TcpStream,
    buffer: [u8; 1024],
}

impl TcpStreamHandler {
    pub fn new(source: TcpStream) -> Self {
        Self {
            source: source,
            buffer: [0; 1024],
        }
    }
    pub async fn next_request(&mut self) -> Result<Request, Box<dyn Error>> {
        let mut is_end_of_line = false;
        let mut data: Vec<u8> = vec![];

        while !is_end_of_line {
            if let Ok(n) = self.source.read(&mut self.buffer).await {
                if n == 0 {
                    info!("Stream closed");
                    break;
                }
                data.append(&mut self.buffer[0..n].into());

                is_end_of_line = char::from(self.buffer[n - 1]) == '\n';
            } else {
                error!("An error occurred");
            };
        }

        Ok(serde_json::from_slice(&data)?)
    }

    pub async fn write_all(&mut self, src: &[u8]) -> Result<(), std::io::Error> {
        self.source.write_all(src).await
    }

    pub async fn shutdown(&mut self) {
        self.source.shutdown().await.unwrap();
    }
}

pub struct ConnectionHandler;
impl ConnectionHandler {
    /// Spawns a tokio task which handles this connection and can be managed with [cancellation_token].
    pub async fn spawn(
        tcp_stream: TcpStream,
        tcp_addr: SocketAddr,
        state: Arc<Mutex<ServerState>>,
        cancellation_token: &CancellationToken,
    ) {
        let _ = tokio::spawn(async move {
            // Handle each connection
            info!("Received new connection: {}", tcp_addr);

            let tcp_stream_handler = Arc::new(Mutex::new(TcpStreamHandler::new(tcp_stream)));

            loop {
                let request = {
                    match tcp_stream_handler.lock().await.next_request().await {
                        Ok(req) => req,
                        Err(e) => {
                            let message = format!("Could not parse request {}", e);
                            error!("{}", message);
                            continue;
                        }
                    }
                };

                let response = match request {
                    Request::Insert(key, value) => state.lock().await.write(&key, value).await,
                    Request::Delete(key) => state.lock().await.delete(&key).await,
                    Request::Read(key) => state.lock().await.read(&key).await,
                };
                tcp_stream_handler
                    .lock()
                    .await
                    .write_all((serde_json::to_string_pretty(&response).unwrap() + "\n").as_bytes())
                    .await
                    .unwrap();
            }
        })
        .with_cancellation_token(cancellation_token);
    }
}

mod test {
    use std::net::{Ipv4Addr, SocketAddrV4};

    use super::*;

    const CONNECT_TO: fn(u16) -> SocketAddr =
        |port| SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port));

    async fn create_mock_source(data: &[u8], port: u16) -> TcpStream {
        let source = tokio::net::TcpListener::bind(CONNECT_TO(port))
            .await
            .unwrap();

        tokio::net::TcpSocket::new_v4()
            .unwrap()
            .connect(CONNECT_TO(port))
            .await
            .unwrap()
            .write_all(data)
            .await
            .unwrap();

        return source.accept().await.unwrap().0;
    }

    #[tokio::test]
    async fn next_request_parses_simple_request() {
        let req = Request::Read("test\n".into());
        let mut handler = TcpStreamHandler::new(
            create_mock_source(&mut serde_json::to_vec(&req).unwrap().as_slice(), 12345).await,
        );

        assert_eq!(handler.next_request().await.unwrap(), req);
        handler.shutdown().await;
    }

    #[tokio::test]
    async fn next_request_invalid_data_errors() {
        let mut handler =
            TcpStreamHandler::new(create_mock_source("unparsable text\n".as_bytes(), 12346).await);

        assert!(handler.next_request().await.is_err());
        handler.shutdown().await;
    }
}

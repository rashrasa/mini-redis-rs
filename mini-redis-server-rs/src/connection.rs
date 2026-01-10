use std::{net::SocketAddr, sync::Arc};

use log::{debug, error, info};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Mutex,
};
use tokio_util::{future::FutureExt, sync::CancellationToken};

use crate::{Error, Request, ServerState};

pub struct TcpStreamHandler {
    source: TcpStream,
    buffer: [u8; 1024],
    request_queue: Vec<Result<Request, Error>>,
}

impl TcpStreamHandler {
    pub fn new(source: TcpStream) -> Self {
        Self {
            source: source,
            buffer: [0; 1024],
            request_queue: Vec::with_capacity(16),
        }
    }
    pub async fn next_request(&mut self) -> Result<Request, Error> {
        let mut is_end_of_line = false;
        let mut data: Vec<u8> = vec![];

        while !is_end_of_line {
            let n = self.source.read(&mut self.buffer).await?;

            if n == 0 {
                info!("Stream closed");
                break;
            }
            data.append(&mut self.buffer[0..n].into());

            is_end_of_line = char::from(self.buffer[n - 1]) == '\n';
        }
        let req = str::from_utf8(&data).unwrap();
        debug!("{}", req);
        req.split("\n").for_each(|s| {
            self.request_queue
                .push(serde_json::from_str::<Request>(s).map_err(|e| Error::SerdeJsonError(e)))
        });
        self.request_queue.pop().unwrap() // currently assuming that a request always exists at this point
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

            let mut tcp_stream_handler = TcpStreamHandler::new(tcp_stream);

            loop {
                let request: Request = {
                    match tcp_stream_handler.next_request().await {
                        Ok(req) => req,
                        Err(error) => match error {
                            Error::StdIoError(e) => match e.kind() {
                                std::io::ErrorKind::NotFound
                                | std::io::ErrorKind::PermissionDenied
                                | std::io::ErrorKind::ConnectionRefused
                                | std::io::ErrorKind::ConnectionReset
                                | std::io::ErrorKind::HostUnreachable
                                | std::io::ErrorKind::NetworkUnreachable
                                | std::io::ErrorKind::ConnectionAborted
                                | std::io::ErrorKind::TimedOut
                                | std::io::ErrorKind::StorageFull => {
                                    info!("Stream was closed: {}", e);
                                    break;
                                }
                                _ => {
                                    let message = format!("Could not parse request {}", e);
                                    error!("{}", message);
                                    continue;
                                }
                            },
                            _ => {
                                let message = format!("Could not parse request {}", error);
                                error!("{}", message);
                                continue;
                            }
                        },
                    }
                };

                let response = match request {
                    Request::Insert(key, value) => state.lock().await.write(&key, value).await,
                    Request::Delete(key) => state.lock().await.delete(&key).await,
                    Request::Read(key) => state.lock().await.read(&key).await,
                };
                tcp_stream_handler
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

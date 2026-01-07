use std::{error::Error, net::SocketAddr, sync::Arc};

use log::{error, info};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::Mutex,
};
use tokio_util::{future::FutureExt, sync::CancellationToken};

use crate::Request;

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
}

pub struct Connection {
    tcp_stream: Arc<Mutex<TcpStream>>,
    tcp_addr: Arc<SocketAddr>,
}

impl Connection {
    /// Spawns a tokio task which handles this connection and can be managed with [cancellation_token].
    pub async fn spawn(
        tcp_stream: TcpStream,
        tcp_addr: SocketAddr,
        cancellation_token: &CancellationToken,
    ) -> Self {
        let tcp_stream = Arc::new(Mutex::new(tcp_stream));
        let tcp_addr = Arc::new(tcp_addr);
        let tcp_stream_task = tcp_stream.clone();
        let tcp_addr_task = tcp_addr.clone();
        let _ = tokio::spawn(async move {
            let tcp_stream = tcp_stream_task;
            let tcp_addr = tcp_addr_task;
            // Handle each connection
            info!("Received new connection: {}", tcp_addr);

            let mut buffer: [u8; 1024] = [0; 1024];
            loop {
                let mut tcp_stream = tcp_stream.lock().await;
                tcp_stream.readable().await.unwrap();

                if let Ok(bytes_received) = tcp_stream.read(&mut buffer).await {
                    if bytes_received > 0 {
                        tcp_stream.flush().await.unwrap();
                        info!(
                            "Received tcp message: \n{}\nSize: {} bytes",
                            str::from_utf8(&buffer[0..bytes_received])
                                .unwrap()
                                .replace("\r\n", "")
                                .replace("\n", ""),
                            bytes_received
                        );
                    } else {
                        info!("{} no longer writing, terminating", tcp_addr);
                        tcp_stream.shutdown().await.unwrap();
                        break;
                    }
                }
            }
        })
        .with_cancellation_token(cancellation_token);

        Self {
            tcp_addr,
            tcp_stream,
        }
    }
}

mod test {
    use std::{
        net::{Ipv4Addr, SocketAddrV4},
        pin::Pin,
    };

    use super::*;

    const CONNECT_TO: SocketAddr =
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 6559));

    async fn create_mock_source(data: &[u8]) -> TcpStream {
        let mut source = tokio::net::TcpListener::bind(CONNECT_TO).await.unwrap();

        tokio::net::TcpSocket::new_v4()
            .unwrap()
            .connect(CONNECT_TO)
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
            create_mock_source(&mut serde_json::to_vec(&req).unwrap().as_slice()).await,
        );

        assert_eq!(handler.next_request().await.unwrap(), req);
    }

    #[tokio::test]
    async fn next_request_invalid_data_errors() {
        let mut handler =
            TcpStreamHandler::new(create_mock_source("unparsable text\n".as_bytes()).await);

        assert!(handler.next_request().await.is_err());
    }
}

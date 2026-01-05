use std::{error::Error, net::SocketAddr, sync::Arc};

use log::info;
use tokio::{io::AsyncWriteExt, net::TcpStream, select, sync::Mutex, task::JoinHandle};
use tokio_util::{future::FutureExt, sync::CancellationToken};

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
                if let Ok(bytes_received) = tcp_stream.try_read(&mut buffer) {
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

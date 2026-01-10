use core::f64;
use std::{
    error::Error,
    fmt::format,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
};

use futures::{FutureExt, StreamExt, stream::FuturesUnordered};
use log::{debug, error, info, warn};
use mini_redis_server_rs::Request;
use rand::RngCore;
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::{Mutex, RwLock, mpsc::Sender},
    time::Instant,
};
use tokio_util::bytes::BufMut;

const CONNECT_TO: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 3000));
const TOLERANCE: f64 = 1e-2;
const INITIAL_N: u64 = 50;
// average requests fulfilled per second will be averaged across this window
const EVAL_WINDOW_SECONDS: f64 = 5.0;
// number of consecutive stable evaluation windows before deciding that requests handled / sec has stabilized
const PATIENCE: usize = 5;
const NUM_CONNECTIONS: usize = 20;
const REQUEST_STORE_SIZE: usize = 1024;
const BUFFER_SIZES: usize = 1024 * 10;

async fn read_and_dump_result(read_half: &mut OwnedReadHalf, buffer: &mut [u8; BUFFER_SIZES]) {
    let mut is_end_of_line = false;
    let mut data: Vec<u8> = vec![];

    while !is_end_of_line {
        if let Ok(n) = read_half.read(buffer).await {
            if n == 0 {
                info!("Stream closed");
                break;
            }
            data.put_slice(&buffer[0..n]);

            is_end_of_line = char::from(buffer[n - 1]) == '\n';
        } else {
            error!("An error occurred");
        };
    }

    serde_json::from_slice::<Option<Value>>(&data).unwrap(); //checks for valid response
}

#[tokio::test]
async fn progressive_stress_test() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .target(env_logger::Target::Stdout)
        .init();
    // PURPOSE: This is for evaluating the performance of mini-redis-server-rs. It does not test for correct values after all the inserts.
    // 1. Connect successfully to server on a consistent machine (get hardware info)
    // 2. Start sending n requests per second and wait for response rate to stabilize
    // 3. If response rate ~ n, double n, else end test and record metrics
    // 4. Plot results using matplotlib

    info!("Creating request store");
    let mut rng = rand::rng();
    let request_store: Vec<Vec<u8>> = (0..REQUEST_STORE_SIZE)
        .map(|i| {
            let mut data = serde_json::to_vec(&Request::Insert(
                format!("request_{}", i),
                rng.next_u64().into(),
            ))
            .unwrap();
            data.push('\n' as u8);
            data
        })
        .collect();

    info!("Connecting to server with {} instances", NUM_CONNECTIONS);

    let mut window_fulfilled_request_count: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));

    let mut connection_pool_sender_channels =
        Vec::<tokio::sync::mpsc::Sender<Vec<u8>>>::with_capacity(NUM_CONNECTIONS);

    for i in 0..NUM_CONNECTIONS {
        let (receiver, sender) = tokio::net::TcpStream::connect(CONNECT_TO)
            .await
            .unwrap()
            .into_split();
        let (tx_data, rx_data) = tokio::sync::mpsc::channel::<Vec<u8>>(BUFFER_SIZES);
        connection_pool_sender_channels.push(tx_data);

        info!("Creating reader and writer tasks");

        // Request sender task
        tokio::spawn(async move {
            let mut rx = rx_data;
            let mut sender = sender;
            loop {
                let request = rx.recv().await.unwrap();

                sender.write_all(&request).await.unwrap();
                sender.flush().await.unwrap();
                debug!("Sent request");
            }
        });
        let window_fulfilled_request_count_task = window_fulfilled_request_count.clone();
        tokio::spawn(async move {
            let window_fulfilled_request_count = window_fulfilled_request_count_task;
            let mut buffer: [u8; BUFFER_SIZES] = [0; BUFFER_SIZES];
            let mut receiver = receiver;

            loop {
                read_and_dump_result(&mut receiver, &mut buffer).await;
                *window_fulfilled_request_count.write().await += 1;
            }
        });
    }

    let mut request_rate: u64 = INITIAL_N;
    let mut windows_decreasing: usize = 0;
    let mut behind: f64 = 0.0; // number of requests behind as a float
    let mut last_behind: f64 = f64::INFINITY; // last "behind" count to see if machine or implementation is too slow for stress testing
    let mut last = Instant::now();
    let mut i: usize = 0;

    info!(
        "Starting stress test at {} requests per second",
        request_rate
    );
    loop {
        behind += last.elapsed().as_secs_f64() * (request_rate as f64);
        last = Instant::now();

        while behind > 1.0 {
            if behind > last_behind + 5.0 {
                // padding of 5.0 to filter out small fluctuations
                warn!(
                    "Request \"behind\" count increased: {} -> {}. Use a different machine or close other processes if this continues.",
                    last_behind, behind
                );
                last_behind = behind;
            }
            // Send request
            connection_pool_sender_channels
                .get(i % NUM_CONNECTIONS)
                .unwrap()
                .send(request_store.get(i % REQUEST_STORE_SIZE).unwrap().clone())
                .await
                .unwrap();

            i += 1;
            behind -= 1.0;
        }

        // Evaluate performance and stopping condition
    }
}

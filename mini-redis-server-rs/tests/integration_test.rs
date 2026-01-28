use core::f64;
use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use futures::{StreamExt, stream::FuturesUnordered};
use log::{debug, error, info, warn};
use mini_redis_server_rs::Request;
use rand::RngCore;
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::tcp::OwnedReadHalf,
    select,
    sync::{Mutex, RwLock},
    time::Instant,
};
use tokio_util::bytes::BufMut;

const CONNECT_TO: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 3000));

const STABILITY_TOLERANCE: f64 = 100.0; // Any 'x requests fulfilled' values within this range are considered the same (compared to a previous iteration)
const WITHIN_N_TOLERANCE: f64 = 100.0; // Any 'x requests fulfilled' values within this range are considered the same (compared to a specific n)
const BEHIND_TOLERANCE: f64 = 100.0;
const INITIAL_N: f64 = 100000.0;
// average requests fulfilled per second will be averaged across this window
const EVAL_WINDOW_SECONDS: f64 = 5.0;
// number of consecutive stable evaluation windows before deciding that requests handled / sec has stabilized
const PATIENCE: usize = 5;
const NUM_CONNECTIONS: usize = 100;
const REQUEST_STORE_SIZE: usize = 1024;
const BATCH_SIZE: u64 = 100;

async fn read_and_dump_result(read_half: &mut BufReader<OwnedReadHalf>) {
    let mut v = String::new();
    if let Ok(n) = read_half.read_line(&mut v).await {
        if n == 0 {
            info!("Stream closed");
        }
    } else {
        error!("An error occurred");
    };
}

#[tokio::test]
// PURPOSE: This is for evaluating the performance of mini-redis-server-rs. It does not test for correct values after all the inserts.
// 1. Connect successfully to server on a consistent machine (get hardware info)
// 2. Start sending n requests per second and wait for response rate to stabilize
// 3. If response rate ~ n, double n, else end test and record metrics
// 4. Plot results using matplotlib
async fn progressive_stress_test() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .target(env_logger::Target::Stdout)
        .init();

    // HOW IT WORKS:
    //
    // Tokio Threads: Connections run and continuously check the desired request send rate per connection and attempt to match it.
    //
    // OS Thread: Receives "request sent" messages from each tokio thread
    // OS Thread: Receives responses from the server from each connection
    // OS Thread: Manages the send rate and evaluates performance

    warn!("Initializing tokio-console");
    console_subscriber::init();

    info!("Connecting to server with {} instances", NUM_CONNECTIONS);

    let window_response_count: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));
    let total_response_count: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));

    let mut request_rate: f64 = INITIAL_N;
    let mut windows_not_increasing: usize = 0;
    let mut window_average_handled: [f64; PATIENCE] = [0.0; PATIENCE];
    let mut max_avg_fulfilled_count: f64 = 0.0;

    info!("Creating reader and writer tasks");
    let (request_sent_tx, mut request_sent_rx) = tokio::sync::mpsc::channel::<u64>(1024);
    let (send_rate_tx, send_rate_rx) = tokio::sync::watch::channel::<f64>(0.0);

    info!(
        "Starting stress test at {} requests per second",
        request_rate
    );
    let total_response_count_thread = total_response_count.clone();
    std::thread::spawn(move || {
        let total_response_count = total_response_count_thread.clone();
        let interval = Duration::new(1, 0);
        let count = Arc::new(std::sync::RwLock::new(0u64));

        let count_reader = count.clone();
        std::thread::spawn(move || {
            let count = count_reader;
            loop {
                info!(
                    "Requests sent: {}, Responses received: {}",
                    count.read().unwrap(),
                    total_response_count.blocking_read()
                );
                std::thread::sleep(interval);
            }
        });

        loop {
            *count.write().unwrap() += request_sent_rx.blocking_recv().unwrap();
        }
    });

    // Send first request rate
    send_rate_tx.send(request_rate).unwrap();

    for _ in 0..NUM_CONNECTIONS {
        let (receiver, sender) = tokio::net::TcpStream::connect(CONNECT_TO)
            .await
            .unwrap()
            .into_split();
        let send_rate_rx_task = send_rate_rx.clone();
        let request_sent_tx_task = request_sent_tx.clone();

        tokio::spawn(async move {
            // Request sender task
            let task_requests_sent_count_tx = request_sent_tx_task.clone();
            let mut request_store: [Vec<u8>; REQUEST_STORE_SIZE / NUM_CONNECTIONS] =
                [const { vec![] }; REQUEST_STORE_SIZE / NUM_CONNECTIONS];

            for i in 0..(REQUEST_STORE_SIZE / NUM_CONNECTIONS) {
                let mut data = vec![];
                for _ in 0..BATCH_SIZE {
                    data.extend(
                        serde_json::to_vec(&Request::Insert(format!("request_{}", i), i.into()))
                            .unwrap(),
                    );
                    data.push('\n' as u8);
                }

                request_store[i] = data;
            }
            let request_store = request_store;
            let requests_sent_count_tx = task_requests_sent_count_tx;

            let mut sender = sender;
            let mut commands_rx = send_rate_rx_task;

            let mut behind: f64 = 0.0; // number of requests behind as a float
            let mut last_behind: f64 = f64::INFINITY; // last "behind" count to see if machine or implementation is too slow for stress testing
            let mut last = Instant::now();

            // Receive commands for request rate
            let mut k: usize = 0;
            loop {
                let request_rate = *commands_rx.borrow_and_update() / NUM_CONNECTIONS as f64;

                behind += last.elapsed().as_secs_f64() * request_rate;
                last = Instant::now();

                while behind > BATCH_SIZE as f64 {
                    if behind > last_behind + BEHIND_TOLERANCE {
                        warn!(
                            "\"Behind\" count increased: {:.1} -> {:.1}. Use a different machine or close other processes if this continues.",
                            last_behind, behind
                        );
                    }
                    last_behind = behind;
                    // Send requests
                    let req_i = k % (REQUEST_STORE_SIZE / NUM_CONNECTIONS);
                    let request = &request_store[req_i];
                    sender.write_all(&request).await.unwrap();
                    k += 1;

                    behind -= BATCH_SIZE as f64;
                    requests_sent_count_tx.send(BATCH_SIZE).await.unwrap();
                }

                tokio::task::yield_now().await;
            }
        });

        let window_response_count_task = window_response_count.clone();
        let total_response_count_task = total_response_count.clone();
        tokio::spawn(async move {
            let window_fulfilled_request_count = window_response_count_task;
            let total_fulfilled_request_count = total_response_count_task;
            let mut receiver = BufReader::new(receiver);

            loop {
                read_and_dump_result(&mut receiver).await;
                *window_fulfilled_request_count.write().await += 1;
                *total_fulfilled_request_count.write().await += 1;
            }
        });
    }

    // Evaluate performance and stopping condition
    loop {
        let fulfilled = *window_response_count.read().await;
        let avg_fulfilled = fulfilled as f64 / EVAL_WINDOW_SECONDS;
        info!(
            "Average requests fulfilled per second for current window: {:.2}",
            avg_fulfilled
        );
        if avg_fulfilled < max_avg_fulfilled_count + STABILITY_TOLERANCE {
            window_average_handled[windows_not_increasing] = avg_fulfilled;
            windows_not_increasing += 1;
            if windows_not_increasing >= PATIENCE {
                // average requests fulfillment rate has stabilized
                if avg_fulfilled >= request_rate - WITHIN_N_TOLERANCE {
                    // stabilized around N
                    request_rate *= 2.0;
                    windows_not_increasing = 0;
                    send_rate_tx.send(request_rate).unwrap();
                    info!(
                        "Requests handled / second stabilized around {:.2}, increasing request rate to {}",
                        window_average_handled.iter().sum::<f64>() / PATIENCE as f64,
                        request_rate
                    )
                } else {
                    // stabilized outside N
                    info!(
                        "Requests handled / second saturated around {:.2}, stopping stress test.",
                        window_average_handled.iter().sum::<f64>() / PATIENCE as f64
                    );
                    return;
                }
            }
        } else {
            max_avg_fulfilled_count = avg_fulfilled;
            windows_not_increasing = 0;
        }

        *window_response_count.write().await = 0;
        std::thread::sleep(Duration::new(EVAL_WINDOW_SECONDS as u64, 0));
    }
}

use core::f64;
use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use log::{debug, error, info, warn};
use mini_redis_server_rs::Request;
use rand::RngCore;
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
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
const INITIAL_N: f64 = 1000.0;
// average requests fulfilled per second will be averaged across this window
const EVAL_WINDOW_SECONDS: f64 = 5.0;
// number of consecutive stable evaluation windows before deciding that requests handled / sec has stabilized
const PATIENCE: usize = 5;
const NUM_CONNECTIONS: usize = 1000;
const REQUEST_STORE_SIZE: usize = 1024;
const BUFFER_SIZES: usize = 1024 * 1;

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
        .filter_level(log::LevelFilter::Info)
        .target(env_logger::Target::Stdout)
        .init();
    // PURPOSE: This is for evaluating the performance of mini-redis-server-rs. It does not test for correct values after all the inserts.
    // 1. Connect successfully to server on a consistent machine (get hardware info)
    // 2. Start sending n requests per second and wait for response rate to stabilize
    // 3. If response rate ~ n, double n, else end test and record metrics
    // 4. Plot results using matplotlib

    warn!("Initializing tokio-console");
    console_subscriber::init();

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

    let window_fulfilled_request_count: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));
    let total_fulfilled_request_count: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));

    let mut connection_pool_sender_channels =
        Vec::<tokio::sync::mpsc::Sender<Vec<u8>>>::with_capacity(NUM_CONNECTIONS);

    info!("Creating reader and writer tasks");
    let (requests_sent_count_tx, mut requests_sent_count_rx) =
        tokio::sync::mpsc::channel::<()>(1024);
    for _ in 0..NUM_CONNECTIONS {
        let (receiver, sender) = tokio::net::TcpStream::connect(CONNECT_TO)
            .await
            .unwrap()
            .into_split();
        let (tx_data, rx_data) = tokio::sync::mpsc::channel::<Vec<u8>>(BUFFER_SIZES);
        connection_pool_sender_channels.push(tx_data);

        // Request sender task
        let task_requests_sent_count_tx = requests_sent_count_tx.clone();
        tokio::spawn(async move {
            let requests_sent_count_tx = task_requests_sent_count_tx;
            let mut rx = rx_data;
            let mut sender = sender;
            loop {
                let request = rx.recv().await.unwrap();

                sender.write_all(&request).await.unwrap();
                sender.flush().await.unwrap();
                requests_sent_count_tx.send(()).await.unwrap();
            }
        });
        let window_fulfilled_request_count_task = window_fulfilled_request_count.clone();
        let total_fulfilled_request_count_task = total_fulfilled_request_count.clone();
        tokio::spawn(async move {
            let window_fulfilled_request_count = window_fulfilled_request_count_task;
            let total_fulfilled_request_count = total_fulfilled_request_count_task;
            let mut buffer: [u8; BUFFER_SIZES] = [0; BUFFER_SIZES];
            let mut receiver = receiver;

            loop {
                read_and_dump_result(&mut receiver, &mut buffer).await;
                *window_fulfilled_request_count.write().await += 1;
                *total_fulfilled_request_count.write().await += 1;
            }
        });
    }

    let mut request_rate: f64 = INITIAL_N;
    let mut windows_not_increasing: usize = 0;
    let mut window_average_handled: [f64; PATIENCE] = [0.0; PATIENCE];
    let mut max_avg_fulfilled_count: f64 = 0.0;
    let mut behind: f64 = 0.0; // number of requests behind as a float
    let mut last_behind: f64 = f64::INFINITY; // last "behind" count to see if machine or implementation is too slow for stress testing
    let mut last = Instant::now();
    let mut last_check = Instant::now();
    let mut i: usize = 0;

    info!(
        "Starting stress test at {} requests per second",
        request_rate
    );

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::new(1, 0));
        let mut count = 0u64;
        loop {
            select! {
                _ = interval.tick() => {
                    info!("Requests sent: {}, Responses received: {}", count, total_fulfilled_request_count.read().await);
                }
                _ = requests_sent_count_rx.recv() => {
                    count+=1;
                }
            };
        }
    });

    // TODO: Fix 'while behind' loop to eventually yield if too busy, then set 'last_behind' value
    loop {
        behind += last.elapsed().as_secs_f64() * (request_rate as f64);
        last = Instant::now();

        while behind > 1.0 {
            if behind > last_behind + BEHIND_TOLERANCE {
                warn!(
                    "Request \"behind\" count increased: {} -> {}. Use a different machine or close other processes if this continues.",
                    last_behind, behind
                );
            }
            last_behind = behind;
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
        if last_check.elapsed().as_secs_f64() > EVAL_WINDOW_SECONDS {
            let mut fulfilled = window_fulfilled_request_count.write().await;
            let avg_fulfilled = *fulfilled as f64 / EVAL_WINDOW_SECONDS;
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

            last_check = Instant::now();
            *fulfilled = 0;
        }
        tokio::task::yield_now().await;
    }
}

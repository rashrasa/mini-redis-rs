use core::f64;
use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use log::{info, warn};
use mini_redis_rs::Request;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpStream, tcp::OwnedReadHalf},
    select,
    time::Instant,
};

const CONNECT_TO: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 2, 30), 3000));

const STABILITY_TOLERANCE: f64 = 1000.0; // Any 'x requests fulfilled' values within this range are considered the same (compared to a previous iteration)
const WITHIN_N_TOLERANCE: f64 = 1000.0; // Any 'x requests fulfilled' values within this range are considered the same (compared to a specific n)
const BEHIND_TOLERANCE: f64 = 1000.0;
const INITIAL_N: f64 = 600000.0;
// average requests fulfilled per second will be averaged across this window
const EVAL_WINDOW_SECONDS: f64 = 5.0;
// number of consecutive stable evaluation windows before deciding that requests handled / sec has stabilized
const PATIENCE: usize = 5;
const NUM_CONNECTIONS: usize = 16;
const REQUEST_STORE_SIZE: usize = 1024;
const BATCH_SIZE: u64 = 10000;
const BUFFERED_READER_CAP: usize = 2 * 1024;

async fn read_and_dump_result(read_half: &mut BufReader<OwnedReadHalf>) -> Result<usize, String> {
    let buf = read_half.fill_buf().await.unwrap();
    let n = buf.iter().filter(|b| **b == b'\n').count();
    let len = buf.len();
    read_half.consume(len);
    Ok(n)
}

// PURPOSE: This is for evaluating the performance of mini-redis-server-rs. It does not test for correct values after all the inserts.
// 1. Connect successfully to server on a consistent machine (get hardware info)
// 2. Start sending n requests per second and wait for response rate to stabilize
// 3. If response rate ~ n, double n, else end test and record metrics
// 4. Plot results using matplotlib
#[tokio::main(flavor = "multi_thread", worker_threads = 16)]
async fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .target(env_logger::Target::Stdout)
        .init();

    // HOW IT WORKS:
    //
    // N TCP connections are started
    //
    // Connections have their own tokio task

    // warn!("Initializing tokio-console");
    // console_subscriber::init();

    let window_sent_count: &'static _ = Box::leak(Box::new(AtomicU64::new(0)));
    let window_response_count: &'static _ = Box::leak(Box::new(AtomicU64::new(0)));

    info!("Creating request store");
    let mut request_store: Vec<Vec<u8>> = vec![];
    for _ in 0..REQUEST_STORE_SIZE / NUM_CONNECTIONS {
        let mut data = vec![];
        for i in 0..BATCH_SIZE {
            data.extend(
                serde_json::to_vec(&Request::Insert(format!("request_{}", i), i.into())).unwrap(),
            );
            data.push('\n' as u8);
        }
        request_store.push(data);
    }

    // Declared here since send tasks start here.
    let mut eval_interval = tokio::time::interval(Duration::from_secs_f64(EVAL_WINDOW_SECONDS));

    let (rate_tx, rate_rx) = tokio::sync::watch::channel(INITIAL_N / NUM_CONNECTIONS as f64);

    info!("Connecting to server with {} instances", NUM_CONNECTIONS);
    let rate_rx_task = rate_rx.clone();
    for _ in 0..NUM_CONNECTIONS {
        let mut rate_rx = rate_rx_task.clone();
        let request_store_task = request_store.clone();
        tokio::spawn(async move {
            let request_store = request_store_task;

            let mut k: usize = 0;

            let (reader, mut writer) = TcpStream::connect(CONNECT_TO).await.unwrap().into_split();

            tokio::spawn(async move {
                let mut reader = BufReader::with_capacity(BUFFERED_READER_CAP, reader);
                loop {
                    let r = read_and_dump_result(&mut reader).await.unwrap();
                    window_response_count.fetch_add(r as u64, Ordering::Relaxed);
                }
            });

            let mut behind = 0.0;
            let mut last_behind = f64::MAX;
            let mut last = Instant::now();
            let mut request_rate: f64 = INITIAL_N / NUM_CONNECTIONS as f64;

            // Sender loop
            // Ensures it's on schedule, sending at the broadcasted send rate
            loop {
                behind += last.elapsed().as_secs_f64() * request_rate;
                last = Instant::now();

                select! {
                    _ = async {
                        while behind > BATCH_SIZE as f64 {
                            // Send requests
                            let req_i = k % (REQUEST_STORE_SIZE / NUM_CONNECTIONS);
                            let request = &request_store[req_i];
                            writer.write_all(&request).await.unwrap();
                            k += 1;

                            behind -= BATCH_SIZE as f64;
                            window_sent_count.fetch_add(BATCH_SIZE, Ordering::Relaxed);
                        }
                        last_behind = behind;
                    } => {}
                    _ = rate_rx.changed() => {
                        request_rate = *rate_rx.borrow_and_update();
                    }
                }
                tokio::task::yield_now().await;
            }
        });
    }

    info!("Starting stress test at {} requests per second", INITIAL_N);

    // Evaluation task
    let mut sent: u64 = 0;
    let mut received: u64 = 0;
    let mut max_avg_fulfilled_count: f64 = 0.0;
    let mut request_rate: f64 = INITIAL_N;
    let mut behind = 0.0;
    let mut last_behind = f64::MAX;

    let mut windows_not_increasing: usize = 0;
    let mut window_average_handled: [f64; PATIENCE] = [0.0; PATIENCE];
    loop {
        select! {
            _ = eval_interval.tick() => {

            }

            _ = tokio::signal::ctrl_c() => {
                warn!("Cancelling tests");
                break;
            }
        }

        let last_window_receive_count = window_response_count.swap(0, Ordering::Relaxed);
        let last_window_send_count = window_sent_count.swap(0, Ordering::Relaxed);

        received += last_window_receive_count;
        sent += last_window_send_count;

        behind += request_rate * EVAL_WINDOW_SECONDS - last_window_send_count as f64;

        let avg_fulfilled_rate = last_window_receive_count as f64 / EVAL_WINDOW_SECONDS;

        if behind > last_behind + BEHIND_TOLERANCE {
            warn!(
                "\"Behind\" count increased: {:.1} -> {:.1}. Use a different machine or close other processes if this continues.",
                last_behind, behind
            );
        }
        last_behind = behind;

        info!("Average response rate in window: {:.2}", avg_fulfilled_rate);
        if avg_fulfilled_rate < max_avg_fulfilled_count + STABILITY_TOLERANCE {
            window_average_handled[windows_not_increasing] = avg_fulfilled_rate;
            windows_not_increasing += 1;
            if windows_not_increasing >= PATIENCE {
                // average requests fulfillment rate has stabilized
                if avg_fulfilled_rate >= request_rate - WITHIN_N_TOLERANCE {
                    // stabilized around N
                    request_rate *= 2.0;
                    windows_not_increasing = 0;
                    info!(
                        "Requests handled / second stabilized around {:.2}, increasing request rate to {}",
                        window_average_handled.iter().sum::<f64>() / PATIENCE as f64,
                        request_rate
                    );
                    rate_tx.send(request_rate / NUM_CONNECTIONS as f64).unwrap();
                } else {
                    // stabilized outside N
                    info!(
                        "Requests handled / second saturated around {:.2}, stopping stress test.",
                        window_average_handled.iter().sum::<f64>() / PATIENCE as f64
                    );
                    break;
                }
            }
        } else {
            max_avg_fulfilled_count = avg_fulfilled_rate;
            windows_not_increasing = 0;
        }
    }
    println!("Total Sent: {}\n Total Received: {}", sent, received);
}

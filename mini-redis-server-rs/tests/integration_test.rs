use core::f64;
use std::{
    io::Read, net::{Ipv4Addr, SocketAddr, SocketAddrV4}, sync::Arc, time::Duration
};

use log::{ error, info, warn};
use mini_redis_server_rs::Request;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpStream, tcp::OwnedReadHalf},
    select,
    sync::{RwLock},
    time::{Instant},
};

const CONNECT_TO: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 3000));

const STABILITY_TOLERANCE: f64 = 100.0; // Any 'x requests fulfilled' values within this range are considered the same (compared to a previous iteration)
const WITHIN_N_TOLERANCE: f64 = 100.0; // Any 'x requests fulfilled' values within this range are considered the same (compared to a specific n)
const BEHIND_TOLERANCE: f64 = 100.0;
const INITIAL_N: f64 = 100000.0;
// average requests fulfilled per second will be averaged across this window
const EVAL_WINDOW_SECONDS: f64 = 5.0;
// number of consecutive stable evaluation windows before deciding that requests handled / sec has stabilized
const PATIENCE: usize = 5;
const NUM_CONNECTIONS: usize = 4;
const REQUEST_STORE_SIZE: usize = 1024;
const BATCH_SIZE: u64 = 1000;

#[derive(Debug)]
pub struct MiniTestStatusSnapshot {
    time: Instant,
    target_sent_rate: f64,
    send_rate: f64,
    response_rate: f64,
}

#[derive(Debug)]
pub struct MiniTestResult {
    start: Instant,
    snapshots: Vec<MiniTestStatusSnapshot>,
}

async fn read_and_dump_result(read_half: &mut BufReader<OwnedReadHalf>) ->Result<(), String> {
    let mut v = String::new();
    if let Ok(n) = read_half.read_line(&mut v).await {
        if n == 0 {
            return Err("Stream closed".into());
        }
    } else {
        error!("An error occurred");
    };
    Ok(())
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
    // A bunch of mini stress tests occur per connection
    // Many connections are created
    // Results are joined at the end
    // Stopping conditions are evaluated individually per connection
    // Connections have their own tokio task

    // warn!("Initializing tokio-console");
    // console_subscriber::init();

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

    info!("Connecting to server with {} instances", NUM_CONNECTIONS);
    let results = (0..NUM_CONNECTIONS).map(|c|{
        let request_store_task = request_store.clone();
        tokio::spawn(async move {
            let request_store = request_store_task;

            let mut sent: u64 = 0;
            let mut max_avg_fulfilled_count: f64 = 0.0;
            let mut request_rate: f64 = INITIAL_N;

            let mut windows_not_increasing: usize = 0;
            let mut window_average_handled: [f64; PATIENCE] = [0.0; PATIENCE];
            let mut window_sent = 0_u64;

            let mut behind: f64 = 0.0; // number of requests behind as a float
            let mut last_behind: f64 = f64::INFINITY; // last "behind" count to see if machine or implementation is too slow for stress testing
            let mut last = Instant::now();

            let mut k: usize = 0;

            let mut data = MiniTestResult {
                start: Instant::now(),
                snapshots: vec![],
            };
            let (reader, mut writer) = TcpStream::connect(CONNECT_TO).await.unwrap().into_split();

            let window_response_count = Arc::new(RwLock::new(0_u64));
            let total_response_count = Arc::new(RwLock::new(0_u64));

            let window_response_count_reader = window_response_count.clone();
            let total_response_count_reader = total_response_count.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(reader);
                let window_response_count = window_response_count_reader;
                let total_response_count = total_response_count_reader;

                loop {
                    read_and_dump_result(&mut reader).await.unwrap();
                    *window_response_count.write().await += 1;
                    *total_response_count.write().await += 1;
                }
            });
            let mut last_eval = Instant::now();
            let mut catch_up_window = tokio::time::interval(Duration::new(3, 0));
            loop {
                behind += last.elapsed().as_secs_f64() * request_rate;
                last = Instant::now();

                
                    select! {
                        _ = catch_up_window.tick() => {
                            warn!("Cannot send requests at the desired rate: Expected: {}, Actual: {}", request_rate, (request_rate -( last_behind - behind)) / 3.0);
                        }
                        _ = async {
                            while behind > BATCH_SIZE as f64 {
                                // Send requests
                                let req_i = k % (REQUEST_STORE_SIZE / NUM_CONNECTIONS);
                                let request = &request_store[req_i];
                                writer.write_all(&request).await.unwrap();
                                k += 1;

                                behind -= BATCH_SIZE as f64;
                                sent += BATCH_SIZE;
                                window_sent += BATCH_SIZE;
                            }
                        } => {}
                    }
                    
                if behind > last_behind + BEHIND_TOLERANCE {
                    warn!(
                        "\"Behind\" count increased: {:.1} -> {:.1} for connection {}. Use a different machine or close other processes if this continues.",
                        last_behind, behind, c
                    );
                }
                last_behind = behind;

                tokio::task::yield_now().await;
                if last_eval.elapsed().as_secs_f64() > EVAL_WINDOW_SECONDS {
                    last_eval = Instant::now();
                    let mut window_response_count = window_response_count.write().await;
                    let avg_fulfilled = *window_response_count as f64 / EVAL_WINDOW_SECONDS;
                    info!(
                        "Average response rate in window: {:.2}",
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
                                break;
                            }
                        }
                    } else {
                        max_avg_fulfilled_count = avg_fulfilled;
                        windows_not_increasing = 0;
                    }

                    data.snapshots.push(
                        MiniTestStatusSnapshot { 
                            time: last_eval, 
                            target_sent_rate: request_rate,
                            send_rate: window_sent as f64 / EVAL_WINDOW_SECONDS, 
                            response_rate: *window_response_count as f64 / EVAL_WINDOW_SECONDS
                        }
                    );

                    *window_response_count = 0;
                    window_sent = 0;
                    
                }
            }
            data
        })
    });

    info!("Starting stress test at {} requests per second", INITIAL_N);

    select! {
        results = futures::future::join_all(results) => {
            info!("Test Complete");

            // let mut commit_id = String::new();
            // std::process::Command::new(
            //     "git log -1 --format=%H"
            // ).spawn().unwrap().stdout.unwrap().read_to_string(&mut commit_id).unwrap();
            for result in results.iter() {
                info!("Result: {:?}", result);
            }
        }

        _ = tokio::signal::ctrl_c() => {
            warn!("Cancelling tests");
        }
    }

    
}

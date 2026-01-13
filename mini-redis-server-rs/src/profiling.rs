use std::{
    iter::Map,
    ops::{Add, AddAssign, Deref, Div, DivAssign, Mul, MulAssign, Sub, SubAssign},
    slice::Iter,
    sync::Arc,
    time::{Duration, SystemTime},
};

use log::warn;
use tokio::{sync::Mutex, time::Instant};

mod collector;

use crate::{
    Error,
    profiling::collector::{TimestampedDataCollector, TimestampedDataHandler},
};

#[derive(Clone)]
struct TaskInfo {
    idle: bool,
    cumulative_idle_percentage: f64,
    total_running_time_secs: f64,
    size_gb: f64,
}

#[derive(Clone)]
struct ProfilingData {
    num_cores: usize,
    core_usages: Vec<f64>,
    total_ram_gb: f64,
    current_ram_usage_gb: f64,
    num_tasks: usize,
    task_info: Vec<TaskInfo>,
    connection_latency_ms: f64,
    d_responses_handled: usize,
}

pub struct MiniRedisProfiler {
    collector: TimestampedDataHandler<ProfilingData>,
}

impl MiniRedisProfiler {
    pub fn spawn() -> Self {
        let cached_n_cores = std::thread::available_parallelism().unwrap();
        todo!();
        // Self {
        //     collector: TimestampedDataCollector::spawn(
        //         60.0,
        //         Arc::new(|t| ProfilingData {
        //             num_cores: cached_n_cores.into(),
        //             core_usages: (),
        //             total_ram_gb: (),
        //             current_ram_usage_gb: (),
        //             num_tasks: (),
        //             task_info: (),
        //             connection_latency_ms: (),
        //             d_responses_handled: (),
        //         }),
        //     ),
        // }
    }
}

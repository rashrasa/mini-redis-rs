use std::{
    iter::Map,
    ops::{Add, AddAssign, Deref, Div, DivAssign, Mul, MulAssign, Sub, SubAssign},
    slice::Iter,
    sync::Arc,
    time::{Duration, SystemTime},
};

use log::warn;
use tokio::{sync::Mutex, time::Instant};

use crate::Error;

struct TimestampedDataInstance<T>
where
    T: Send + Sync + Clone + 'static,
{
    data: T,
    time: Instant,
}
pub struct TimestampedData<T>
where
    T: Send + Sync + Clone + 'static,
{
    data: Vec<TimestampedDataInstance<T>>,
    hz: u64,
    sample: Arc<dyn (Fn(Duration) -> T) + Send + Sync>,
    sampling: Arc<Mutex<bool>>,
    start: Instant,
}

impl<T> TimestampedData<T>
where
    T: Send + Sync + Clone + 'static,
{
    /// Spawns an OS thread which samples performance metrics at a regular interval.
    ///
    /// 'sample' needs to return an owned type. It provides a Duration representing the amount of time since the thread was spawned.
    ///
    /// Use as low of a sampling rate as possible to minimize the performance footprint of data collection.
    ///
    /// There is no guarantee that data points are exactly 1/hz seconds apart. Samples may get dropped.
    /// This thread also sleeps between intervals.
    pub fn spawn(hz: u64, sample: Arc<dyn (Fn(Duration) -> T) + Send + Sync>) {
        std::thread::spawn(move || {
            let mut _samples: u64 = 0;
            let mut _dropped: u64 = 0;
            let samples = |s: &u64, d: &u64| s + d;
            let mut data_store = Self {
                data: vec![],
                hz: hz,
                sample: sample,
                sampling: Arc::new(Mutex::new(false)),
                start: Instant::now(),
            };
            let start = &data_store.start;
            loop {
                let elapsed = start.elapsed();
                let behind =
                    elapsed.as_secs_f64() * hz as f64 - samples(&_samples, &_dropped) as f64;
                if behind > 1.0 {
                    data_store.data.push(TimestampedDataInstance {
                        data: (data_store.sample)(elapsed),
                        time: Instant::now(),
                    });
                    _samples += 1;

                    let to_drop = (behind - 1.0) as u64;
                    if to_drop > 1 {
                        _dropped += to_drop;
                        warn!(
                            "Dropped {} samples. Consider reducing the sampling rate if this continues to be a problem.",
                            to_drop
                        );
                    }
                }
                std::thread::sleep(Duration::new(
                    0,
                    (((((samples(&_samples, &_dropped) + 1) as f64) * 1.0 / hz as f64)
                        - start.elapsed().as_secs_f64())
                        * 1e9) as u32,
                ))
            }
        });
    }

    fn get_iter_in(
        &self,
        from: Instant,
        window: Duration,
    ) -> Result<Iter<'_, TimestampedDataInstance<T>>, Error> {
        let to = from + window;
        if window == Duration::ZERO {
            return Err(Error::CrateError(
                "\"from\" needs to be before \"to\"".into(),
            ));
        }
        let start_idx_inc = match self.data.binary_search_by(|a| a.time.cmp(&from)) {
            Ok(n) => n,
            Err(n) => n,
        };
        let end_idx_inc = match self.data.binary_search_by(|a| a.time.cmp(&to)) {
            Ok(n) => n,
            Err(n) => n - 1,
        };
        if end_idx_inc < start_idx_inc {
            return Err(Error::CrateError("Could not find any elements".into()));
        }
        Ok(self.data[start_idx_inc..=end_idx_inc].iter())
    }

    pub fn stop(&mut self) {
        *self.sampling.blocking_lock() = true;
    }
    pub fn sort(&mut self) {
        self.data.sort()
    }
}

// Methods for types T that can be compared and used in arithmetic.
// These methods are computationally expensive and should be used after finishing data collection
impl<T> TimestampedData<T>
where
    T: Send + Sync + Clone + 'static + AddAssign + DivAssign<usize>,
{
    /// Implementation of arithmetic traits (AddAssign, DivAssign, etc.) should be done individually for each field.
    ///
    /// Example:
    ///
    /// ```
    /// pub fn add_assign(&mut self, rhs: MyDataType) {
    ///     self.field_1 += rhs.field_1;
    ///     self.field_2 += rhs.field_2;
    ///     self.field_3 += rhs.field_3;
    /// }
    /// ```
    pub fn average(&self, from: Instant, window: Duration) -> Result<T, Error> {
        let mut values = self.get_iter_in(from, window)?;
        let n = values.len();
        if n == 0 {
            return Err(Error::CrateError("Could not find any elements".into()));
        }
        let mut result = values.next().unwrap().data.clone();
        for value in values {
            result += value.data.clone();
        }
        result /= n;
        Ok(result)
    }

    /// Finds the <percentile * 100>th percentile in the given window. Ensure the percentile argument is in (0.0, 1.0).
    /// Implementation of arithmetic traits (AddAssign, DivAssign, etc.) should be done individually for each field.
    ///
    /// Example:
    ///
    /// ```
    /// pub fn add_assign(&mut self, rhs: MyDataType) {
    ///     self.field_1 += rhs.field_1;
    ///     self.field_2 += rhs.field_2;
    ///     self.field_3 += rhs.field_3;
    /// }
    /// ```
    pub fn find_percentile(
        &self,
        from: Instant,
        window: Duration,
        percentile: f32,
    ) -> Result<T, Error> {
        todo!()
    }
}

impl<T> PartialEq for TimestampedDataInstance<T>
where
    T: Send + Sync + Clone + 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl<T> Eq for TimestampedDataInstance<T> where T: Send + Sync + Clone + 'static {}

impl<T> PartialOrd for TimestampedDataInstance<T>
where
    T: Send + Sync + Clone + 'static,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.time.partial_cmp(&other.time)
    }
}
impl<T> Ord for TimestampedDataInstance<T>
where
    T: Send + Sync + Clone + 'static,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time.cmp(&other.time)
    }
}

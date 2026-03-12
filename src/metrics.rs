use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounter, IntGauge, register_histogram,
    register_histogram_vec, register_int_counter, register_int_gauge,
};
use std::sync::LazyLock;

pub static COUNTER: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!("requests_total", "Total requests")
        .expect("Failed to register intcounter")
});
pub static GAUGE: LazyLock<IntGauge> = LazyLock::new(|| {
    register_int_gauge!("active_connections", "Active number of permits")
        .expect("Failed to initialize IntGauge")
});

pub static PARSE_LATENCY: LazyLock<Histogram> = LazyLock::new(|| {
    let buckets = vec![0.0001, 0.00025, 0.0005, 0.001, 0.0025, 0.005, 0.01];

    let opts = HistogramOpts::new("parse_latency_histogram", "Histogram of parse latency")
        .buckets(buckets);

    register_histogram!(opts).expect("Failed to create histogram")
});

pub static PROCESS_LATENCY: LazyLock<HistogramVec> = LazyLock::new(|| {
    let buckets = vec![0.0001, 0.00025, 0.0005, 0.001, 0.0025, 0.005, 0.01];

    let opts = HistogramOpts::new("process_latency_histogram", "Histogram of process latency")
        .buckets(buckets);

    register_histogram_vec!(opts, &["command"]).expect("Failed to create histogram")
});

pub static WRITE_LATENCY: LazyLock<Histogram> = LazyLock::new(|| {
    let buckets = vec![0.0001, 0.00025, 0.0005, 0.001, 0.0025, 0.005, 0.01];

    let opts = HistogramOpts::new("write_latency_histogram", "Histogram of write latency")
        .buckets(buckets);

    register_histogram!(opts).expect("Failed to create histogram")
});

pub static DB_LATENCY: LazyLock<HistogramVec> = LazyLock::new(|| {
    let buckets: Vec<f64> = vec![0.00001, 0.000025, 0.00005, 0.0001, 0.00025, 0.0005, 0.001];

    let opts = HistogramOpts::new("db_lock_latency_histogram", "Histogram for db lock latency")
        .buckets(buckets);

    register_histogram_vec!(opts, &["command"]).expect("Failed to create histogram")
});

pub struct ActiveConnectionGuard;

impl Default for ActiveConnectionGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveConnectionGuard {
    pub fn new() -> Self {
        GAUGE.inc();
        ActiveConnectionGuard {}
    }
}

impl Drop for ActiveConnectionGuard {
    fn drop(&mut self) {
        GAUGE.dec();
    }
}

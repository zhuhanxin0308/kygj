use std::time::Duration;

/// 协议与资源保护限额集中声明；不是科研精度或许可配额。
pub const PROTOCOL_VERSION: u32 = 1;
pub const SCHEMA_VERSION: u32 = 2;
pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_STDERR_BYTES: usize = 512 * 1024;
pub const MAX_TOTAL_SAMPLES: usize = 100_000;
pub const MAX_RAYS: usize = 256;
pub const MIN_SAMPLES: usize = 2;
pub const MIN_RELATIVE_TOLERANCE: f64 = 100.0 * f64::EPSILON;
pub const DEFAULT_WALL_TIME_SECONDS: u64 = 300;
pub const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(20);
pub const PROCESS_TERMINATION_TIMEOUT: Duration = Duration::from_secs(3);
pub const SHUTDOWN_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(10);
pub const DESCRIBE_TIMEOUT: Duration = Duration::from_secs(15);
pub const STREAM_CHUNK_BYTES: usize = 8192;
pub const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// 测试可以注入短时限；生产预检必须把实际采用的限制展示给研究者。
#[derive(Debug, Clone)]
pub struct ProcessLimits {
    pub max_wall_time: Duration,
    pub max_output_bytes: usize,
    pub max_stderr_bytes: usize,
    pub poll_interval: Duration,
    pub termination_timeout: Duration,
}

impl Default for ProcessLimits {
    fn default() -> Self {
        Self { max_wall_time: Duration::from_secs(DEFAULT_WALL_TIME_SECONDS), max_output_bytes: MAX_RESPONSE_BYTES, max_stderr_bytes: MAX_STDERR_BYTES, poll_interval: PROCESS_POLL_INTERVAL, termination_timeout: PROCESS_TERMINATION_TIMEOUT }
    }
}

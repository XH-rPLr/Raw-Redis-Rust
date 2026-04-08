use dotenvy::dotenv;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Redis server bind address
    #[serde(default = "default_redis_bind_addr")]
    pub redis_bind_addr: String,

    /// API/Metrics server bind address
    #[serde(default = "default_api_bind_addr")]
    pub api_bind_addr: String,

    /// Maximum number of concurrent connections
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    /// Idle connection timeout in seconds
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,

    /// TCP listen backlog size
    #[serde(default = "default_listen_backlog")]
    pub listen_backlog: u32,
}

fn default_redis_bind_addr() -> String {
    "0.0.0.0:6379".to_string()
}

fn default_api_bind_addr() -> String {
    "0.0.0.1:9091".to_string()
}

fn default_max_connections() -> usize {
    1024
}

fn default_idle_timeout_secs() -> u64 {
    30
}

fn default_listen_backlog() -> u32 {
    1024
}

impl Config {
    pub fn load() -> Result<Self, envy::Error> {
        dotenv().ok();

        envy::from_env::<Config>()
    }

    pub fn idle_timeout(&self) -> Duration {
        Duration::from_secs(self.idle_timeout_secs)
    }
}

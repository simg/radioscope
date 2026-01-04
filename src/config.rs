use std::env;
use std::path::PathBuf;

#[allow(dead_code)]
pub struct AppConfig {
    pub monitor_interface: String,
    pub http_bind: String,
    pub tick_frequency_hz: f32,
    pub tick_duration_ms: u64,
    pub tick_volume: f32,
    pub db_path: PathBuf,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            monitor_interface: env_var("MONITOR_INTERFACE", "wlan1mon"),
            http_bind: env_var("HTTP_BIND", "0.0.0.0:8080"),
            tick_frequency_hz: env_var("TICK_FREQUENCY_HZ", "880").parse().unwrap_or(880.0),
            tick_duration_ms: env_var("TICK_DURATION_MS", "20").parse().unwrap_or(20),
            tick_volume: env_var("TICK_VOLUME", "0.35").parse().unwrap_or(0.35),
            db_path: env_var("DB_PATH", "/var/lib/radioscope/radioscope.sqlite").into(),
        }
    }
}

fn env_var(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

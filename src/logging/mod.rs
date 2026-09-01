use anyhow::Result;
use std::path::PathBuf;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

pub fn init_logging() -> Result<PathBuf> {
    let log_dir = dirs_log_path();
    std::fs::create_dir_all(&log_dir)?;

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("cleaner=debug,info"));

    let stdout_layer = fmt::layer().with_ansi(true).with_target(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .try_init()
        .ok();

    tracing::info!("Logging initialized. Log directory: {:?}", log_dir);
    Ok(log_dir)
}

pub fn dirs_log_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("cleaner")
        .join("logs")
}

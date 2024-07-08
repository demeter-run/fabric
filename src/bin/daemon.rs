use std::env;

use anyhow::Result;
use dotenv::dotenv;
use serde::Deserialize;
use tracing::Level;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let env_filter = EnvFilter::builder()
        .with_default_directive(Level::INFO.into())
        .with_env_var("RUST_LOG")
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(env_filter)
        .init();

    let config = Config::new()?;

    fabric::drivers::monitor::subscribe(&config.brokers).await
}

#[derive(Debug, Deserialize)]
struct Config {
    brokers: String,
}
impl Config {
    pub fn new() -> Result<Self> {
        let config = config::Config::builder()
            .add_source(
                config::File::with_name(&env::var("DAEMON_CONFIG").unwrap_or("daemon.toml".into()))
                    .required(false),
            )
            .add_source(config::Environment::with_prefix("daemon").separator("_"))
            .build()?
            .try_deserialize()?;

        Ok(config)
    }
}

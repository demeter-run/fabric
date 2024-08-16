use std::{collections::HashMap, env};

use anyhow::Result;
use dotenv::dotenv;
use fabric::drivers::monitor::MonitorConfig;
use serde::Deserialize;
use tokio::try_join;
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

    let schedule = fabric::drivers::cron::schedule();
    let subscribe = fabric::drivers::monitor::subscribe(config.into());

    try_join!(schedule, subscribe)?;

    Ok(())
}

#[derive(Debug, Deserialize)]
struct Config {
    topic: String,
    kafka: HashMap<String, String>,
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

impl From<Config> for MonitorConfig {
    fn from(value: Config) -> Self {
        Self {
            kafka: value.kafka,
            topic: value.topic,
        }
    }
}

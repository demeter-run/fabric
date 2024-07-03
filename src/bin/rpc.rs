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

    futures::future::try_join(
        fabric::drivers::grpc::server(&config.addr, &config.db_path, &config.brokers),
        fabric::drivers::event::subscribe(&config.brokers),
    )
    .await?;

    Ok(())
}

#[derive(Debug, Deserialize)]
struct Config {
    addr: String,
    db_path: String,
    brokers: String,
}
impl Config {
    pub fn new() -> Result<Self> {
        let config = config::Config::builder()
            .add_source(
                config::File::with_name(&env::var("RPC_CONFIG").unwrap_or("rpc.toml".into()))
                    .required(false),
            )
            .add_source(config::Environment::with_prefix("rpc").separator("_"))
            .build()?
            .try_deserialize()?;

        Ok(config)
    }
}

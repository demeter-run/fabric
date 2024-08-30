use std::{collections::HashMap, env, path::PathBuf};

use anyhow::Result;
use dotenv::dotenv;
use fabric::drivers::{cache::CacheConfig, grpc::GrpcConfig};
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

    let grpc = fabric::drivers::grpc::server(config.clone().into());
    let subscribe = fabric::drivers::cache::subscribe(config.clone().into());

    try_join!(grpc, subscribe)?;

    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct Auth {
    url: String,
}
#[derive(Debug, Clone, Deserialize)]
struct Stripe {
    url: String,
    api_key: String,
}
#[derive(Debug, Clone, Deserialize)]
struct Config {
    addr: String,
    db_path: String,
    crds_path: PathBuf,
    auth: Auth,
    stripe: Stripe,
    secret: String,
    topic: String,
    kafka_producer: HashMap<String, String>,
    kafka_consumer: HashMap<String, String>,
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

impl From<Config> for GrpcConfig {
    fn from(value: Config) -> Self {
        Self {
            addr: value.addr,
            db_path: value.db_path,
            crds_path: value.crds_path,
            auth_url: value.auth.url,
            stripe_url: value.stripe.url,
            stripe_api_key: value.stripe.api_key,
            secret: value.secret,
            kafka: value.kafka_producer,
            topic: value.topic,
        }
    }
}

impl From<Config> for CacheConfig {
    fn from(value: Config) -> Self {
        Self {
            kafka: value.kafka_consumer,
            db_path: value.db_path,
            topic: value.topic,
        }
    }
}

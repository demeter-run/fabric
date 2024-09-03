use std::{collections::HashMap, env, path::PathBuf, time::Duration};

use anyhow::Result;
use dotenv::dotenv;
use fabric::drivers::{cache::CacheConfig, grpc::GrpcConfig};
use serde::{de::Visitor, Deserialize, Deserializer};
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
    client_id: String,
    client_secret: String,
    audience: String,
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
    #[serde(deserialize_with = "deserialize_duration")]
    #[serde(rename(deserialize = "invite_ttl_min"))]
    invite_ttl: Duration,
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
            auth_client_id: value.auth.client_id,
            auth_client_secret: value.auth.client_secret,
            auth_audience: value.auth.audience,
            stripe_url: value.stripe.url,
            stripe_api_key: value.stripe.api_key,
            secret: value.secret,
            kafka: value.kafka_producer,
            topic: value.topic,
            invite_ttl: value.invite_ttl,
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

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_map(DurationVisitor)
}

struct DurationVisitor;
impl<'de> Visitor<'de> for DurationVisitor {
    type Value = Duration;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("This Visitor expects to receive i64 minutes")
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Duration::from_secs(v as u64))
    }
}

use std::{collections::HashMap, env, sync::Arc, time::Duration};

use anyhow::Result;
use dotenv::dotenv;
use fabric::{
    driven::prometheus::metrics::MetricsDriven,
    drivers::{cache::CacheConfig, monitor::MonitorConfig, usage::UsageConfig},
};
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
    let metrics_driven = Arc::new(MetricsDriven::new()?);

    let metrics = fabric::drivers::metrics::server(&config.metrics.addr, metrics_driven.clone());

    match config.mode {
        Mode::Usage => {
            let cache = fabric::drivers::cache::subscribe(config.clone().into());
            let usage =
                fabric::drivers::usage::schedule(config.clone().into(), metrics_driven.clone());

            try_join!(cache, usage, metrics)?;
        }
        Mode::Monitor => {
            let monitor =
                fabric::drivers::monitor::subscribe(config.clone().into(), metrics_driven.clone());

            try_join!(monitor, metrics)?;
        }
        Mode::Full => {
            let cache = fabric::drivers::cache::subscribe(config.clone().into());
            let usage =
                fabric::drivers::usage::schedule(config.clone().into(), metrics_driven.clone());
            let monitor =
                fabric::drivers::monitor::subscribe(config.clone().into(), metrics_driven.clone());

            try_join!(cache, usage, monitor, metrics)?;
        }
    };

    Ok(())
}

#[derive(Debug, Deserialize, Clone)]
enum Mode {
    Usage,
    Monitor,
    Full,
}

#[derive(Debug, Deserialize, Clone)]
struct Prometheus {
    url: String,
    query_step: String,
}

#[derive(Debug, Deserialize, Clone)]
struct Metrics {
    addr: String,
}

#[derive(Debug, Deserialize, Clone)]
struct Config {
    db_path: String,
    cluster_id: String,
    prometheus: Prometheus,
    metrics: Metrics,
    #[serde(deserialize_with = "deserialize_duration")]
    #[serde(rename(deserialize = "delay_sec"))]
    delay: Duration,
    topic_events: String,
    topic_usage: String,
    kafka_producer: HashMap<String, String>,
    kafka_monitor: HashMap<String, String>,
    kafka_cache: HashMap<String, String>,
    mode: Mode,
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
            kafka: value.kafka_monitor,
            topic: value.topic_events,
        }
    }
}

impl From<Config> for UsageConfig {
    fn from(value: Config) -> Self {
        Self {
            db_path: value.db_path,
            cluster_id: value.cluster_id,
            prometheus_url: value.prometheus.url,
            prometheus_query_step: value.prometheus.query_step,
            delay: value.delay,
            kafka: value.kafka_producer,
            topic: value.topic_usage,
        }
    }
}

impl From<Config> for CacheConfig {
    fn from(value: Config) -> Self {
        Self {
            kafka: value.kafka_cache,
            db_path: value.db_path,
            topics: [value.topic_events, value.topic_usage].to_vec(),
            notify: None,
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
impl Visitor<'_> for DurationVisitor {
    type Value = Duration;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("This Visitor expects to receive i64 seconds")
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Duration::from_secs(v as u64))
    }
}

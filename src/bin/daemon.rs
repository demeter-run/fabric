use std::{collections::HashMap, env, time::Duration};

use anyhow::Result;
use dotenv::dotenv;
use fabric::drivers::{monitor::MonitorConfig, usage::UsageConfig};
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

    match config.mode {
        Some(Mode::Usage) => {
            fabric::drivers::usage::schedule(config.clone().into()).await?;
        }
        Some(Mode::Monitor) => {
            fabric::drivers::monitor::subscribe(config.clone().into()).await?;
        }
        None => {
            let schedule = fabric::drivers::usage::schedule(config.clone().into());
            let subscribe = fabric::drivers::monitor::subscribe(config.clone().into());

            try_join!(schedule, subscribe)?;
        }
    };

    Ok(())
}

#[derive(Debug, Deserialize, Clone)]
enum Mode {
    Usage,
    Monitor,
}

#[derive(Debug, Deserialize, Clone)]
struct Prometheus {
    url: String,
    query_step: String,
}

#[derive(Debug, Deserialize, Clone)]
struct Config {
    cluster_id: String,
    prometheus: Prometheus,
    #[serde(deserialize_with = "deserialize_duration")]
    #[serde(rename(deserialize = "delay_sec"))]
    delay: Duration,
    topic: String,
    kafka: HashMap<String, String>,
    mode: Option<Mode>,
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

impl From<Config> for UsageConfig {
    fn from(value: Config) -> Self {
        Self {
            cluster_id: value.cluster_id,
            prometheus_url: value.prometheus.url,
            prometheus_query_step: value.prometheus.query_step,
            delay: value.delay,
            kafka: value.kafka,
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
        formatter.write_str("This Visitor expects to receive i64 seconds")
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Duration::from_secs(v as u64))
    }
}

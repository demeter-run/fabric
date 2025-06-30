use std::sync::Arc;
use std::{collections::HashMap, env, path::PathBuf, time::Duration};

use anyhow::Result;
use dotenv::dotenv;
use fabric::driven::prometheus::metrics::MetricsDriven;
use fabric::drivers::{
    cache::{CacheConfig, CacheNotifyConfig},
    grpc::{GrpcConfig, GrpcTlsConfig},
};
use serde::{de::Visitor, Deserialize, Deserializer};
use tokio::try_join;
use tracing::Level;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install default provider");
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

    let grpc = fabric::drivers::grpc::server(config.clone().into(), metrics_driven.clone());
    let subscribe = fabric::drivers::cache::subscribe(config.clone().into());
    let metrics = fabric::drivers::metrics::server(&config.prometheus.addr, metrics_driven.clone());

    try_join!(grpc, subscribe, metrics)?;

    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct AuthConfig {
    url: String,
    client_id: String,
    client_secret: String,
    audience: String,
}
#[derive(Debug, Clone, Deserialize)]
struct StripeConfig {
    url: String,
    api_key: String,
}
#[derive(Debug, Clone, Deserialize)]
struct EmailConfig {
    #[serde(deserialize_with = "deserialize_duration")]
    #[serde(rename(deserialize = "invite_ttl_min"))]
    invite_ttl: Duration,
    ses_access_key_id: String,
    ses_secret_access_key: String,
    ses_region: String,
    ses_verified_email: String,
}
#[derive(Debug, Clone, Deserialize)]
struct TlsConfig {
    ssl_crt_path: PathBuf,
    ssl_key_path: PathBuf,
}
#[derive(Debug, Clone, Deserialize)]
struct PrometheusConfig {
    addr: String,
}
#[derive(Debug, Clone, Deserialize)]
struct BaliusConfig {
    pg_url: String,
    vault_address: String,
    vault_token: String,
}
#[derive(Debug, Clone, Deserialize)]
struct Config {
    addr: String,
    db_path: String,
    crds_path: PathBuf,
    auth: AuthConfig,
    email: EmailConfig,
    stripe: StripeConfig,
    secret: String,
    topic_events: String,
    topic_usage: String,
    tls: Option<TlsConfig>,
    slack_webhook_url: Option<String>,
    kafka_producer: HashMap<String, String>,
    kafka_consumer: HashMap<String, String>,
    prometheus: PrometheusConfig,
    balius: Option<BaliusConfig>,
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
            topic: value.topic_events,
            invite_ttl: value.email.invite_ttl,
            ses_access_key_id: value.email.ses_access_key_id,
            ses_secret_access_key: value.email.ses_secret_access_key,
            ses_region: value.email.ses_region,
            ses_verified_email: value.email.ses_verified_email,
            tls_config: value.tls.map(|value| GrpcTlsConfig {
                ssl_key_path: value.ssl_key_path,
                ssl_crt_path: value.ssl_crt_path,
            }),
            balius_pg_url: value.balius.as_ref().map(|b| b.pg_url.clone()),
            balius_vault_address: value.balius.as_ref().map(|b| b.vault_address.clone()),
            balius_vault_token: value.balius.as_ref().map(|b| b.vault_token.clone()),
        }
    }
}

impl From<Config> for CacheConfig {
    fn from(value: Config) -> Self {
        Self {
            kafka: value.kafka_consumer,
            db_path: value.db_path,
            topics: [value.topic_events, value.topic_usage].to_vec(),
            notify: value.slack_webhook_url.map(|url| CacheNotifyConfig {
                slack_webhook_url: url,
                auth_url: value.auth.url,
                auth_client_id: value.auth.client_id,
                auth_client_secret: value.auth.client_secret,
                auth_audience: value.auth.audience,
            }),
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
        formatter.write_str("This Visitor expects to receive i64 minutes")
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Duration::from_secs(60 * (v as u64)))
    }
}

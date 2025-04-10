use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};

use anyhow::Result;
use chrono::Utc;
use tokio::time::sleep;
use tracing::{error, info};

use crate::{
    domain::{error::Error, usage},
    driven::{
        cache::{usage::SqliteUsageDrivenCache, SqliteCache},
        kafka::KafkaProducer,
        prometheus::{metrics::MetricsDriven, PrometheusUsageDriven},
    },
};

pub async fn schedule(config: UsageConfig, metrics: Arc<MetricsDriven>) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    let usage_cache = Arc::new(SqliteUsageDrivenCache::new(sqlite_cache.clone()));

    let prometheus_driven = Arc::new(PrometheusUsageDriven::new(&config.prometheus_url));
    let event_bridge = Arc::new(KafkaProducer::new(&config.topic, &config.kafka)?);

    let mut cursor = Utc::now();

    info!("Usage schedule running");
    loop {
        sleep(config.delay).await;

        let result = usage::cluster::sync_usage(
            usage_cache.clone(),
            prometheus_driven.clone(),
            event_bridge.clone(),
            &config.cluster_id,
            &config.prometheus_query_step,
            cursor,
        )
        .await;

        match result {
            Ok(total_units) => {
                info!("Successfully sync usage");
                metrics.domain_usage_collected(total_units);
                cursor = Utc::now();
            }
            Err(err) => {
                error!(error = err.to_string(), "Error running sync usage");
                handle_error_metric(metrics.clone(), "usage", &err);
            }
        }
    }
}

pub struct UsageConfig {
    pub db_path: String,
    pub cluster_id: String,
    pub prometheus_url: String,
    pub prometheus_query_step: String,
    pub delay: Duration,
    pub topic: String,
    pub kafka: HashMap<String, String>,
}

fn handle_error_metric(metrics: Arc<MetricsDriven>, domain: &str, error: &Error) {
    if let Error::Unexpected(err) = error {
        metrics.domain_error("usage", domain, &err.to_string());
    }
}

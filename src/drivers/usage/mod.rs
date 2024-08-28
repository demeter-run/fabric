use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use chrono::Utc;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::{
    domain::usage,
    driven::{kafka::KafkaProducer, prometheus::PrometheusUsageDriven},
};

pub async fn schedule(config: UsageConfig) -> Result<()> {
    let prometheus_driven = Arc::new(PrometheusUsageDriven::new(&config.prometheus_url));
    let event_bridge = Arc::new(KafkaProducer::new(&config.topic, &config.kafka)?);

    let mut cursor = Utc::now();

    loop {
        sleep(config.delay).await;

        let result = usage::cluster::sync_usage(
            prometheus_driven.clone(),
            event_bridge.clone(),
            &config.cluster_id,
            cursor,
        )
        .await;

        match result {
            Ok(()) => {
                info!("Successfully sync usage");
                cursor = Utc::now();
            }
            Err(err) => warn!(error = err.to_string(), "Error running sync usage"),
        }
    }
}

pub struct UsageConfig {
    pub cluster_id: String,
    pub prometheus_url: String,
    pub delay: Duration,
    pub topic: String,
    pub kafka: HashMap<String, String>,
}

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::{
    domain::usage,
    driven::{kafka::KafkaProducer, prometheus::PrometheusUsageDriven},
};

pub async fn schedule() -> Result<()> {
    let prometheus_driven = Arc::new(PrometheusUsageDriven::new("").await?);
    let event_bridge = Arc::new(KafkaProducer::new(Default::default(), &Default::default())?);

    // TODO: get cluster id from config file
    let cluster_id = "";

    loop {
        let result =
            usage::cluster::sync_usage(prometheus_driven.clone(), event_bridge.clone(), cluster_id)
                .await;

        match result {
            Ok(()) => info!("Successfully sync usage"),
            Err(err) => warn!(error = err.to_string(), "Error running sync usage"),
        }

        sleep(Duration::from_secs(5)).await;
    }
}

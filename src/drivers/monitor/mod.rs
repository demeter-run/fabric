use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig,
};
use std::{borrow::Borrow, collections::HashMap, sync::Arc};
use tracing::{error, info, warn};

use crate::{
    domain::{error::Error, event::Event, project, resource},
    driven::{k8s::K8sCluster, prometheus::metrics::MetricsDriven},
};

pub async fn subscribe(config: MonitorConfig, metrics: Arc<MetricsDriven>) -> Result<()> {
    let cluster = Arc::new(K8sCluster::new().await?);

    let mut client_config = ClientConfig::new();
    for (k, v) in config.kafka.iter() {
        client_config.set(k, v);
    }
    let consumer: StreamConsumer = client_config.create()?;
    consumer.subscribe(&[&config.topic])?;

    info!("Monitor subscribe running");
    loop {
        match consumer.recv().await {
            Err(error) => error!(?error, "kafka subscribe error"),
            Ok(message) => {
                let message = message.borrow();
                match message.try_into() {
                    Ok(event) => {
                        let result = {
                            match &event {
                                Event::ProjectCreated(evt) => {
                                    project::cluster::apply_manifest(cluster.clone(), evt.clone())
                                        .await
                                        .inspect_err(|err| {
                                            handle_error_metric(metrics.clone(), "project", err)
                                        })
                                }
                                Event::ProjectDeleted(evt) => {
                                    project::cluster::delete_manifest(cluster.clone(), evt.clone())
                                        .await
                                        .inspect_err(|err| {
                                            handle_error_metric(metrics.clone(), "project", err)
                                        })
                                }
                                Event::ResourceCreated(evt) => {
                                    resource::cluster::apply_manifest(cluster.clone(), evt.clone())
                                        .await
                                        .inspect_err(|err| {
                                            handle_error_metric(metrics.clone(), "resource", err)
                                        })
                                }
                                Event::ResourceUpdated(evt) => {
                                    resource::cluster::patch_manifest(cluster.clone(), evt.clone())
                                        .await
                                        .inspect_err(|err| {
                                            handle_error_metric(metrics.clone(), "resource", err)
                                        })
                                }
                                Event::ResourceDeleted(evt) => {
                                    resource::cluster::delete_manifest(cluster.clone(), evt.clone())
                                        .await
                                        .inspect_err(|err| {
                                            handle_error_metric(metrics.clone(), "resource", err)
                                        })
                                }
                                _ => {
                                    info!(event = event.key(), "bypass event");
                                    Ok(())
                                }
                            }
                        };

                        match result {
                            Ok(()) => {
                                info!(event = event.key(), "Successfully handled event")
                            }
                            Err(err) => warn!(
                                event = event.key(),
                                error = err.to_string(),
                                "Error running event."
                            ),
                        }

                        consumer.commit_message(message, CommitMode::Async)?;
                    }
                    Err(error) => {
                        error!(?error, "fail to convert message to event");
                        consumer.commit_message(message, CommitMode::Async)?;
                    }
                }
            }
        };
    }
}

#[derive(Debug)]
pub struct MonitorConfig {
    pub topic: String,
    pub kafka: HashMap<String, String>,
}

fn handle_error_metric(metrics: Arc<MetricsDriven>, domain: &str, error: &Error) {
    if let Error::Unexpected(err) = error {
        metrics.domain_error("monitor", domain, &err.to_string());
    }
}

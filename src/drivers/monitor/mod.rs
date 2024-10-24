use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig, Message,
};
use regex::Regex;
use std::{borrow::Borrow, collections::HashMap, sync::Arc};
use tracing::{error, info, warn};

use crate::{
    domain::{event::Event, project, resource},
    driven::k8s::K8sCluster,
};

pub async fn subscribe(config: MonitorConfig) -> Result<()> {
    let cluster = Arc::new(K8sCluster::new().await?);

    let mut client_config = ClientConfig::new();
    for (k, v) in config.kafka.iter() {
        client_config.set(k, v);
    }
    let consumer: StreamConsumer = client_config.create()?;
    consumer.subscribe(&[&config.topic])?;

    let source_regex = Regex::new(r"fabric-.+")?;

    info!("Monitor subscribe running");
    loop {
        match consumer.recv().await {
            Err(error) => error!(?error, "kafka subscribe error"),
            Ok(message) => {
                let message = message.borrow();

                let payload: serde_json::Value =
                    serde_json::from_slice(message.payload().unwrap_or_default())?;
                match payload.get("annotation") {
                    Some(annotation) => match annotation.get("source") {
                        Some(v) => {
                            let source = v.to_string();
                            if !source_regex.is_match(&source) {
                                info!(?source, "bypass event. Event source not allowed");
                                continue;
                            }
                        }
                        None => {
                            info!("bypass event. Event doesnt have a source");
                            continue;
                        }
                    },
                    None => {
                        info!("bypass event. Event doesnt have a source");
                        continue;
                    }
                };

                match message.try_into() {
                    Ok(event) => {
                        let event_appliclation = {
                            match &event {
                                Event::ProjectCreated(evt) => {
                                    project::cluster::apply_manifest(cluster.clone(), evt.clone())
                                        .await
                                }
                                Event::ProjectDeleted(evt) => {
                                    project::cluster::delete_manifest(cluster.clone(), evt.clone())
                                        .await
                                }
                                Event::ResourceCreated(evt) => {
                                    resource::cluster::apply_manifest(cluster.clone(), evt.clone())
                                        .await
                                }
                                Event::ResourceUpdated(evt) => {
                                    resource::cluster::patch_manifest(cluster.clone(), evt.clone())
                                        .await
                                }
                                Event::ResourceDeleted(evt) => {
                                    resource::cluster::delete_manifest(cluster.clone(), evt.clone())
                                        .await
                                }
                                _ => {
                                    info!(event = event.key(), "bypass event");
                                    Ok(())
                                }
                            }
                        };

                        match event_appliclation {
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

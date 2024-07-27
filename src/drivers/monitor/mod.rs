use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig,
};
use std::{borrow::Borrow, collections::HashMap, sync::Arc};
use tracing::{error, info};

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

    info!("Subscriber running");
    loop {
        match consumer.recv().await {
            Err(error) => error!(?error, "kafka subscribe error"),
            Ok(message) => match message.borrow().try_into() {
                Ok(event) => {
                    match event {
                        Event::ProjectCreated(evt) => {
                            project::apply_manifest(cluster.clone(), evt).await?;
                        }
                        Event::ResourceCreated(evt) => {
                            resource::apply_manifest(cluster.clone(), evt).await?
                        }
                        _ => {
                            info!(event = event.key(), "bypass event")
                        }
                    };
                    consumer.commit_message(&message, CommitMode::Async)?;
                }
                Err(error) => {
                    error!(?error, "fail to convert message to event");
                    consumer.commit_message(&message, CommitMode::Async)?;
                }
            },
        };
    }
}

pub struct MonitorConfig {
    pub topic: String,
    pub kafka: HashMap<String, String>,
}

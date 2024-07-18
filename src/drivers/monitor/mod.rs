use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig,
};
use std::{borrow::Borrow, sync::Arc};
use tracing::{error, info};

use crate::{
    domain::{event::Event, project, resource},
    driven::k8s::K8sCluster,
};

pub async fn subscribe(config: MonitorConfig) -> Result<()> {
    let cluster = Arc::new(K8sCluster::new().await?);

    let topic = String::from("events");

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &config.brokers)
        .set("group.id", &config.consumer_name)
        .create()?;

    consumer.subscribe(&[&topic])?;

    info!("Subscriber running");
    loop {
        match consumer.recv().await {
            Err(err) => error!(error = err.to_string(), "kafka subscribe error"),
            Ok(message) => match message.borrow().try_into() {
                Ok(event) => {
                    match event {
                        Event::ProjectCreated(evt) => {
                            project::apply_cluster(cluster.clone(), evt).await?;
                        }
                        Event::ResourceCreated(evt) => {
                            resource::apply_cluster(cluster.clone(), evt).await?
                        }
                    };
                    consumer.commit_message(&message, CommitMode::Async)?;
                }
                Err(err) => {
                    error!(error = err.to_string(), "fail to convert message to event");
                    consumer.commit_message(&message, CommitMode::Async)?;
                }
            },
        };
    }
}

pub struct MonitorConfig {
    pub brokers: String,
    pub consumer_name: String,
}

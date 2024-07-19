use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig,
};
use std::{borrow::Borrow, collections::HashMap, sync::Arc};
use tracing::{error, info};

use crate::{
    domain::{events::Event, ports, projects},
    driven::k8s::K8sCluster,
};

pub async fn subscribe(config: MonitorConfig) -> Result<()> {
    let k8s_cluster = Arc::new(K8sCluster::new().await?);

    let topic = String::from("events");

    let mut client_config = ClientConfig::new();
    for (k, v) in config.kafka.iter() {
        client_config.set(k, v);
    }
    let consumer: StreamConsumer = client_config.create()?;
    consumer.subscribe(&[&topic])?;

    info!("Subscriber running");
    loop {
        match consumer.recv().await {
            Err(err) => error!(error = err.to_string(), "kafka subscribe error"),
            Ok(message) => match message.borrow().try_into() {
                Ok(event) => {
                    match event {
                        Event::ProjectCreated(namespace) => {
                            projects::create::create_resource(k8s_cluster.clone(), namespace)
                                .await?;
                        }
                        Event::PortCreated(port) => {
                            ports::create::create_resource(k8s_cluster.clone(), port).await?;
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
    pub kafka: HashMap<String, String>,
}

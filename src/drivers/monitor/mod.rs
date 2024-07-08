use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig, Message,
};
use std::sync::Arc;
use tracing::{error, info};

use crate::{
    domain::{daemon::namespace::create_namespace, events::Event},
    driven::k8s::K8sCluster,
};

pub async fn subscribe(brokers: &str) -> Result<()> {
    let k8s_cluster = Arc::new(K8sCluster::new().await?);

    let topic = String::from("events");

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", "clusters")
        .create()?;

    consumer.subscribe(&[&topic])?;

    info!("Subscriber running");
    loop {
        match consumer.recv().await {
            Err(err) => error!(error = err.to_string(), "kafka subscribe error"),
            Ok(message) => {
                if let Some(payload) = message.payload() {
                    let event: Event = serde_json::from_slice(payload)?;
                    match event {
                        Event::ProjectCreated(namespace) => {
                            create_namespace(k8s_cluster.clone(), namespace).await?;
                        }
                        Event::AccountCreated(_) => todo!(),
                        Event::PortCreated(_) => todo!(),
                    };
                    consumer.commit_message(&message, CommitMode::Async)?;
                }
            }
        };
    }
}

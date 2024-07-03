use anyhow::{Error, Result};
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use std::time::Duration;

use crate::domain::events::{Event, EventBridge};

pub struct KafkaProducer {
    producer: FutureProducer,
    topic: String,
}
impl KafkaProducer {
    pub fn new(brokers: &str, topic: &str) -> Result<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .create()?;

        Ok(Self {
            producer,
            topic: topic.to_string(),
        })
    }
}
#[async_trait::async_trait]
impl EventBridge for KafkaProducer {
    async fn dispatch(&self, event: Event) -> Result<()> {
        let data = serde_json::to_vec(&event)?;
        self.producer
            .send(
                FutureRecord::to(&self.topic).payload(&data).key(""),
                Duration::from_secs(0),
            )
            .await
            .map_err(|err| Error::msg(err.0.to_string()))?;

        Ok(())
    }
}

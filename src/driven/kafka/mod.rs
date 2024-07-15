use anyhow::{bail, Error, Result};
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    ClientConfig, Message,
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
        let key = event.key();

        self.producer
            .send(
                FutureRecord::to(&self.topic).payload(&data).key(&key),
                Duration::from_secs(0),
            )
            .await
            .map_err(|err| Error::msg(err.0.to_string()))?;

        Ok(())
    }
}
impl TryFrom<&rdkafka::message::BorrowedMessage<'_>> for Event {
    type Error = Error;

    fn try_from(
        value: &rdkafka::message::BorrowedMessage<'_>,
    ) -> std::result::Result<Self, Self::Error> {
        let Some(key) = value.key() else {
            bail!("event with empty key")
        };
        let key = String::from_utf8(key.to_vec())?;

        let Some(payload) = value.payload() else {
            bail!("event with empty payload")
        };
        let event = Event::from_key(&key, payload)?;
        Ok(event)
    }
}

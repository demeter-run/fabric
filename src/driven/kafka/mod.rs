use anyhow::Result as AnyhowResult;
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    ClientConfig, Message,
};
use std::{collections::HashMap, time::Duration};

use crate::domain::{
    error::Error,
    event::{Event, EventDrivenBridge},
    Result,
};

const NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct KafkaProducer {
    producer: FutureProducer,
    topic: String,
}
impl KafkaProducer {
    pub fn new(topic: &str, properties: &HashMap<String, String>) -> AnyhowResult<Self> {
        let producer: FutureProducer = {
            let mut client_config = ClientConfig::new();
            for (k, v) in properties.iter() {
                client_config.set(k, v);
            }
            client_config.create()?
        };

        Ok(Self {
            producer,
            topic: topic.to_string(),
        })
    }
}
#[async_trait::async_trait]
impl EventDrivenBridge for KafkaProducer {
    async fn dispatch(&self, event: Event) -> Result<()> {
        let annotation = HashMap::from([("source", format!("{NAME}-{VERSION}"))]);

        let Some(mut payload) = serde_json::to_value(&event)?.as_object().cloned() else {
            return Err(Error::Unexpected("invalid event structure".into()));
        };
        payload.insert("annotation".into(), serde_json::to_value(annotation)?);

        let data = serde_json::to_vec(&payload)?;
        let key = event.key();

        self.producer
            .send(
                FutureRecord::to(&self.topic).payload(&data).key(&key),
                Duration::from_secs(0),
            )
            .await
            .map_err(|err| Error::Unexpected(err.0.to_string()))?;

        Ok(())
    }
}
impl TryFrom<&rdkafka::message::BorrowedMessage<'_>> for Event {
    type Error = Error;

    fn try_from(
        value: &rdkafka::message::BorrowedMessage<'_>,
    ) -> std::result::Result<Self, Self::Error> {
        let Some(key) = value.key() else {
            return Err(Error::Unexpected("event with empty key".into()));
        };
        let key =
            String::from_utf8(key.to_vec()).map_err(|err| Error::Unexpected(err.to_string()))?;

        let Some(payload) = value.payload() else {
            return Err(Error::Unexpected("event with empty payload".into()));
        };
        let event = Event::from_key(&key, payload)?;
        Ok(event)
    }
}

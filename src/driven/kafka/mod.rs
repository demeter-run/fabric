use anyhow::{ensure, Result};
use kafka::{
    client::KafkaClient,
    producer::{Producer, Record},
};
use serde_json::json;

use crate::domain::management::events::{Event, EventBridge};

pub struct KafkaEventBridge {
    hosts: Vec<String>,
    topic: String,
}
impl KafkaEventBridge {
    pub fn new(hosts: &[String], topic: &str) -> Result<Self> {
        let hosts = hosts.to_vec();
        let mut client = KafkaClient::new(hosts.to_vec());
        client.load_metadata_all()?;

        let topic = topic.to_string();
        ensure!(
            client.topics().contains(&topic),
            "topic {topic} does not exist yet",
        );

        Ok(Self { hosts, topic })
    }
}
#[async_trait::async_trait]
impl EventBridge for KafkaEventBridge {
    async fn dispatch(&self, _event: Event) -> Result<()> {
        let json = json!({
           "test": "123"
        });
        let value = serde_json::to_vec(&json)?;

        let record = Record::from_key_value(&self.topic, "test".as_bytes(), value);

        let mut producer = Producer::from_hosts(self.hosts.clone()).create()?;
        producer.send(&record)?;

        Ok(())
    }
}

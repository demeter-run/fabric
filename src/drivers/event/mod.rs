use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig, Message,
};
use std::{path::Path, sync::Arc};
use tracing::{error, info};

use crate::{
    domain::{events::Event, management::project::create_cache},
    driven::cache::{project::SqliteProjectCache, SqliteCache},
};

pub async fn subscribe(brokers: &str) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new("dev.db")).await?);
    sqlite_cache.migrate().await?;

    let project_cache = Arc::new(SqliteProjectCache::new(sqlite_cache));

    let topic = String::from("events");

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", "cache")
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
                        Event::NamespaceCreation(namespace) => {
                            create_cache(project_cache.clone(), namespace).await?;
                        }
                        Event::AccountCreation(_) => todo!(),
                    };
                    consumer.commit_message(&message, CommitMode::Async)?;
                }
            }
        };
    }
}

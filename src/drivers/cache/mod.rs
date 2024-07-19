use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig,
};
use std::{borrow::Borrow, path::Path, sync::Arc};
use tracing::{error, info};

use crate::{
    domain::{event::Event, project, resource},
    driven::cache::{
        project::SqliteProjectDrivenCache, resource::SqliteResourceCache, SqliteCache,
    },
};

pub async fn subscribe(config: CacheConfig) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let project_cache = Arc::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));
    let resource_cache = Arc::new(SqliteResourceCache::new(sqlite_cache.clone()));

    let topic = String::from("events");

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &config.brokers)
        .set("group.id", "cache")
        .create()?;

    consumer.subscribe(&[&topic])?;

    info!("Subscriber running");
    loop {
        match consumer.recv().await {
            Err(error) => error!(?error, "kafka subscribe error"),
            Ok(message) => match message.borrow().try_into() {
                Ok(event) => {
                    match event {
                        Event::ProjectCreated(evt) => {
                            project::create_cache(project_cache.clone(), evt).await?;
                        }
                        Event::ResourceCreated(evt) => {
                            resource::create_cache(resource_cache.clone(), evt).await?
                        }
                        Event::ProjectSecretCreated(evt) => {
                            project::create_secret_cache(project_cache.clone(), evt).await?;
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

pub struct CacheConfig {
    pub db_path: String,
    pub brokers: String,
}

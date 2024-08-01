use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig,
};
use std::{borrow::Borrow, collections::HashMap, path::Path, sync::Arc};
use tracing::{error, info};

use crate::{
    domain::{event::Event, project, resource},
    driven::cache::{
        project::SqliteProjectDrivenCache, resource::SqliteResourceDrivenCache, SqliteCache,
    },
};

pub async fn subscribe(config: CacheConfig) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let project_cache = Arc::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));
    let resource_cache = Arc::new(SqliteResourceDrivenCache::new(sqlite_cache.clone()));

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
                            project::cache::create(project_cache.clone(), evt).await?;
                        }
                        Event::ProjectSecretCreated(evt) => {
                            project::cache::create_secret(project_cache.clone(), evt).await?;
                        }
                        Event::ResourceCreated(evt) => {
                            resource::cache::create(resource_cache.clone(), evt).await?
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
    pub topic: String,
    pub kafka: HashMap<String, String>,
}

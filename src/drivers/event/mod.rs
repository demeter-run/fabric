use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig,
};
use std::{borrow::Borrow, collections::HashMap, path::Path, sync::Arc};
use tracing::{error, info};

use crate::{
    domain::{events::Event, ports, projects},
    driven::cache::{port::SqlitePortCache, project::SqliteProjectCache, SqliteCache},
};

pub async fn subscribe(config: EventConfig) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let project_cache = Arc::new(SqliteProjectCache::new(sqlite_cache.clone()));
    let port_cache = Arc::new(SqlitePortCache::new(sqlite_cache.clone()));

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
                            projects::create::create_cache(project_cache.clone(), namespace).await?
                        }
                        Event::PortCreated(port) => {
                            ports::create::create_cache(port_cache.clone(), port).await?
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

pub struct EventConfig {
    pub db_path: String,
    pub kafka: HashMap<String, String>,
}

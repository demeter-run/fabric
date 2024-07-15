use anyhow::{bail, Error, Result};
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig, Message,
};
use std::{borrow::Borrow, path::Path, sync::Arc};
use tracing::{error, info};

use crate::{
    domain::{events::Event, ports, projects, users},
    driven::cache::{
        port::SqlitePortCache, project::SqliteProjectCache, user::SqliteUserCache, SqliteCache,
    },
};

pub async fn subscribe(config: EventConfig) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let project_cache = Arc::new(SqliteProjectCache::new(sqlite_cache.clone()));
    let port_cache = Arc::new(SqlitePortCache::new(sqlite_cache.clone()));
    let user_cache = Arc::new(SqliteUserCache::new(sqlite_cache.clone()));

    let topic = String::from("events");

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &config.brokers)
        .set("group.id", "cache")
        .create()?;

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
                        Event::UserCreated(user) => {
                            users::create::create_cache(user_cache.clone(), user).await?
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

pub struct EventConfig {
    pub db_path: String,
    pub brokers: String,
}

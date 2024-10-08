use anyhow::{bail, Result};
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    error::KafkaError,
    ClientConfig, Message,
};
use std::{borrow::Borrow, collections::HashMap, path::Path, sync::Arc};
use tracing::{error, info, warn};

use crate::{
    domain::{event::Event, notify::NotifyDriven, project, resource, usage},
    driven::{
        auth0::Auth0DrivenImpl,
        cache::{
            project::SqliteProjectDrivenCache, resource::SqliteResourceDrivenCache,
            usage::SqliteUsageDrivenCache, SqliteCache,
        },
        slack::SlackNotifyDrivenImpl,
    },
};

pub async fn subscribe(config: CacheConfig) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new(&config.db_path)).await?);
    sqlite_cache.migrate().await?;

    let project_cache = Arc::new(SqliteProjectDrivenCache::new(sqlite_cache.clone()));
    let resource_cache = Arc::new(SqliteResourceDrivenCache::new(sqlite_cache.clone()));
    let usage_cache = Arc::new(SqliteUsageDrivenCache::new(sqlite_cache.clone()));

    let mut slack_notify_driven = None;
    let mut auth0_driven = None;
    if let Some(notify_config) = config.notify {
        slack_notify_driven = Some(SlackNotifyDrivenImpl::try_new(
            &notify_config.slack_webhook_url,
        )?);
        auth0_driven = Some(Arc::new(
            Auth0DrivenImpl::try_new(
                &notify_config.auth_url,
                &notify_config.auth_client_id,
                &notify_config.auth_client_secret,
                &notify_config.auth_audience,
            )
            .await?,
        ));
    }

    let mut client_config = ClientConfig::new();
    for (k, v) in config.kafka.iter() {
        client_config.set(k, v);
    }

    let consumer: StreamConsumer = client_config.create()?;
    consumer.subscribe(&[&config.topic])?;

    info!("Subscriber running");
    loop {
        let result = consumer.recv().await;
        if let Err(error) = result {
            return match error {
                KafkaError::PartitionEOF(_) => Ok(()),
                _ => bail!(error),
            };
        }

        let message = result.unwrap();

        info!("Consuming from kafka, current offset: {}", message.offset());
        match message.borrow().try_into() {
            Ok(event) => {
                let event_application = match &event {
                    Event::ProjectCreated(evt) => {
                        project::cache::create(project_cache.clone(), evt.clone()).await
                    }
                    Event::ProjectUpdated(evt) => {
                        project::cache::update(project_cache.clone(), evt.clone()).await
                    }
                    Event::ProjectDeleted(evt) => {
                        project::cache::delete(project_cache.clone(), evt.clone()).await
                    }
                    Event::ProjectSecretCreated(evt) => {
                        project::cache::create_secret(project_cache.clone(), evt.clone()).await
                    }
                    Event::ProjectSecretDeleted(evt) => {
                        project::cache::delete_secret(project_cache.clone(), evt.clone()).await
                    }
                    Event::ProjectUserInviteCreated(evt) => {
                        project::cache::create_user_invite(project_cache.clone(), evt.clone()).await
                    }
                    Event::ProjectUserInviteAccepted(evt) => {
                        project::cache::create_user_invite_acceptance(
                            project_cache.clone(),
                            evt.clone(),
                        )
                        .await
                    }
                    Event::ProjectUserDeleted(evt) => {
                        project::cache::delete_user(project_cache.clone(), evt.clone()).await
                    }
                    Event::ResourceCreated(evt) => {
                        resource::cache::create(resource_cache.clone(), evt.clone()).await
                    }
                    Event::ResourceDeleted(evt) => {
                        resource::cache::delete(resource_cache.clone(), evt.clone()).await
                    }
                    Event::UsageCreated(evt) => {
                        usage::cache::create(
                            usage_cache.clone(),
                            resource_cache.clone(),
                            evt.clone(),
                        )
                        .await
                    }
                    Event::ResourceUpdated(evt) => {
                        resource::cache::update(resource_cache.clone(), evt.clone()).await
                    }
                };

                if let Some(notify) = &slack_notify_driven {
                    if let Err(err) = notify
                        .notify(event.clone(), auth0_driven.clone().unwrap().clone())
                        .await
                    {
                        warn!(err = err.to_string(), "Failed to send Slack notification.")
                    }
                }

                match event_application {
                    Ok(_) => info!("Succesfully handled event {:?}", event),
                    Err(err) => error!(
                        error = err.to_string(),
                        "Failed to handle event: {:?}", event
                    ),
                }
                consumer.commit_message(&message, CommitMode::Async)?;
            }
            Err(error) => {
                error!(?error, "fail to convert message to event");
                consumer.commit_message(&message, CommitMode::Async)?;
            }
        };
    }
}

pub struct CacheNotifyConfig {
    pub slack_webhook_url: String,
    pub auth_url: String,
    pub auth_client_id: String,
    pub auth_client_secret: String,
    pub auth_audience: String,
}

pub struct CacheConfig {
    pub db_path: String,
    pub topic: String,
    pub kafka: HashMap<String, String>,
    pub notify: Option<CacheNotifyConfig>,
}

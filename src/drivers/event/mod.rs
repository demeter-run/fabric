use anyhow::Result;
use kafka::{
    client::{FetchOffset, GroupOffsetStorage},
    consumer::Consumer,
};
use std::{path::Path, sync::Arc};
use tracing::info;

use crate::{
    domain::{events::Event, management::project::create_cache},
    driven::cache::{project::SqliteProjectCache, SqliteCache},
};

pub async fn subscribe(kafka_host: &str) -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new("dev.db")).await?);
    sqlite_cache.migrate().await?;

    let project_cache = Arc::new(SqliteProjectCache::new(sqlite_cache));

    let topic = "events".to_string();
    let hosts = &[kafka_host.into()];

    let mut consumer = Consumer::from_hosts(hosts.to_vec())
        .with_topic(topic.clone())
        .with_group("cache".to_string())
        .with_fallback_offset(FetchOffset::Earliest)
        .with_offset_storage(Some(GroupOffsetStorage::Kafka))
        .create()?;

    info!("Subscriber running");

    loop {
        let mss = consumer.poll()?;
        if mss.is_empty() {
            continue;
        }

        for ms in mss.iter() {
            for m in ms.messages() {
                let event: Event = serde_json::from_slice(m.value)?;
                match event {
                    Event::NamespaceCreation(namespace) => {
                        create_cache(project_cache.clone(), namespace).await?;
                    }
                    Event::AccountCreation(_) => todo!(),
                };
            }
            consumer.consume_messageset(ms)?;
        }
        consumer.commit_consumed()?;
    }
}

use anyhow::Result;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::{path::Path, sync::Arc};

use crate::domain::management;
use crate::domain::management::project::Project;
use crate::driven::cache::{project::SqliteProjectCache, SqliteCache};
use crate::driven::kafka::KafkaEventBridge;

pub async fn server() -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new("dev.db")).await?);
    let project_cache = Arc::new(SqliteProjectCache::new(sqlite_cache));

    let event_bridge = Arc::new(KafkaEventBridge::new(&["localhost:9092".into()], "events")?);

    let slug: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();

    let project = Project {
        name: format!("test name {slug}"),
        slug,
    };

    management::project::create(project_cache, event_bridge, project).await?;

    Ok(())
}

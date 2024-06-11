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
    sqlite_cache.migrate().await?;

    let project_state = Arc::new(SqliteProjectCache::new(sqlite_cache));

    let event_bridge = Arc::new(KafkaEventBridge::new(
        &["localhost:19092".into()],
        "events",
    )?);

    let slug: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();

    let project = Project {
        name: "test name".into(),
        slug,
        description: "test description".into(),
    };

    management::project::create(project_state, event_bridge, project).await?;

    Ok(())
}

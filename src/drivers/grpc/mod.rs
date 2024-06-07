use anyhow::Result;
use std::{path::Path, sync::Arc};

use crate::domain::management;
use crate::domain::management::project::Project;
use crate::driven::cache::{project::SqliteProjectCache, SqliteCache};

pub async fn server() -> Result<()> {
    let sqlite_cache = Arc::new(SqliteCache::new(Path::new("dev.db")).await?);
    sqlite_cache.migrate().await?;

    let project_state = Arc::new(SqliteProjectCache::new(sqlite_cache));

    let project = Project {
        name: "test name".into(),
        slug: "test-slug-2".into(),
        description: "test description".into(),
    };

    management::project::create(project_state, project).await?;

    Ok(())
}

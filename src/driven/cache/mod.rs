use anyhow::Result;
use std::path::Path;

pub mod project;
pub mod resource;
pub mod usage;

pub struct SqliteCache {
    db: sqlx::sqlite::SqlitePool,
}

impl SqliteCache {
    pub async fn new(path: &Path) -> Result<Self> {
        let url = format!("sqlite:{}?mode=rwc", path.display());
        let db = sqlx::sqlite::SqlitePoolOptions::new().connect(&url).await?;

        Ok(Self { db })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("src/driven/cache/migrations")
            .run(&self.db)
            .await?;

        Ok(())
    }

    #[cfg(test)]
    pub async fn ephemeral() -> Result<Self> {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await?;

        let out = Self { db };
        out.migrate().await?;

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use resource::SqliteResourceDrivenCache;

    use crate::{
        domain::{
            project::{cache::ProjectDrivenCache, Project},
            resource::{cache::ResourceDrivenCache, Resource},
        },
        driven::cache::project::SqliteProjectDrivenCache,
    };

    use super::*;

    pub async fn mock_project(sqlite_cache: Arc<SqliteCache>) -> Project {
        let cache: Box<dyn ProjectDrivenCache> =
            Box::new(SqliteProjectDrivenCache::new(sqlite_cache));

        let project = Project::default();
        cache.create(&project).await.unwrap();

        project
    }
    pub async fn mock_resource(sqlite_cache: Arc<SqliteCache>, project_id: &str) -> Resource {
        let cache: Box<dyn ResourceDrivenCache> =
            Box::new(SqliteResourceDrivenCache::new(sqlite_cache));

        let resource = Resource {
            project_id: project_id.to_string(),
            ..Default::default()
        };

        cache.create(&resource).await.unwrap();

        resource
    }
}

use anyhow::Result;
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::sync::Arc;

use crate::domain::resource::{ResourceCache, ResourceDrivenCache};

use super::SqliteCache;

pub struct SqliteResourceDrivenCache {
    sqlite: Arc<SqliteCache>,
}
impl SqliteResourceDrivenCache {
    pub fn new(sqlite: Arc<SqliteCache>) -> Self {
        Self { sqlite }
    }
}
#[async_trait::async_trait]
impl ResourceDrivenCache for SqliteResourceDrivenCache {
    async fn create(&self, resource: &ResourceCache) -> Result<()> {
        sqlx::query!(
            r#"
                INSERT INTO resource (id, project_id, kind, data)
                VALUES ($1, $2, $3, $4)
            "#,
            resource.id,
            resource.project_id,
            resource.kind,
            resource.data,
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
}

impl FromRow<'_, SqliteRow> for ResourceCache {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            kind: row.try_get("kind")?,
            data: row.try_get("data")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::project::{ProjectCache, ProjectDrivenCache},
        driven::cache::project::SqliteProjectDrivenCache,
    };

    use super::*;

    async fn mock_project(sqlite_cache: Arc<SqliteCache>) -> ProjectCache {
        let cache: Box<dyn ProjectDrivenCache> =
            Box::new(SqliteProjectDrivenCache::new(sqlite_cache));

        let project = ProjectCache::default();
        cache.create(&project).await.unwrap();

        project
    }

    #[tokio::test]
    async fn it_should_create_resource() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteResourceDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;

        let resource = ResourceCache {
            project_id: project.id,
            ..Default::default()
        };

        let result = cache.create(&resource).await;
        println!("{:?}", result);

        assert!(result.is_ok());
    }
}

use anyhow::Result;
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::sync::Arc;

use crate::domain::resource::{cache::ResourceDrivenCache, Resource};

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
    async fn create(&self, resource: &Resource) -> Result<()> {
        sqlx::query!(
            r#"
                INSERT INTO resource (id, project_id, kind, data, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            resource.id,
            resource.project_id,
            resource.kind,
            resource.data,
            resource.created_at,
            resource.updated_at
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
    async fn find(&self, project_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Resource>> {
        let offset = page_size * (page - 1);

        let resources = sqlx::query_as::<_, Resource>(
            r#"
                SELECT 
                    r.id, 
                	  r.project_id, 
                	  r.kind, 
                	  r.data, 
                	  r.created_at, 
                	  r.updated_at
                FROM resource r
                WHERE r.project_id = $1
                LIMIT $2
                OFFSET $3;
            "#,
        )
        .bind(project_id)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(resources)
    }
}

impl FromRow<'_, SqliteRow> for Resource {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            kind: row.try_get("kind")?,
            data: row.try_get("data")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::project::{cache::ProjectDrivenCache, Project},
        driven::cache::project::SqliteProjectDrivenCache,
    };

    use super::*;

    async fn mock_project(sqlite_cache: Arc<SqliteCache>) -> Project {
        let cache: Box<dyn ProjectDrivenCache> =
            Box::new(SqliteProjectDrivenCache::new(sqlite_cache));

        let project = Project::default();
        cache.create(&project).await.unwrap();

        project
    }

    #[tokio::test]
    async fn it_should_create_resource() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteResourceDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;

        let resource = Resource {
            project_id: project.id,
            ..Default::default()
        };

        let result = cache.create(&resource).await;
        println!("{:?}", result);

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_find_project_resources() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteResourceDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;

        let resource = Resource {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create(&resource).await.unwrap();

        let result = cache.find(&project.id, &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }
    #[tokio::test]
    async fn it_should_return_none_find_project_resources_invalid_page() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteResourceDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;

        let resource = Resource {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create(&resource).await.unwrap();

        let result = cache.find(&project.id, &2, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
    #[tokio::test]
    async fn it_should_return_none_find_project_resources() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteResourceDrivenCache::new(sqlite_cache.clone());

        let result = cache.find(Default::default(), &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}

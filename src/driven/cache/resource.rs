use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::sync::Arc;

use crate::domain::{
    error::Error,
    resource::{cache::ResourceDrivenCache, Resource, ResourceStatus, ResourceUpdate},
    Result,
};

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
    async fn find(&self, project_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Resource>> {
        let offset = page_size * (page - 1);

        let resources = sqlx::query_as::<_, Resource>(
            r#"
                SELECT 
                    r.id, 
                	  r.project_id, 
                	  r.name, 
                	  r.kind, 
                	  r.spec, 
                    r.status,
                	  r.created_at, 
                	  r.updated_at
                FROM resource r
                WHERE r.project_id = $1 and r.status != $2
                ORDER BY r.created_at DESC
                LIMIT $3
                OFFSET $4;
            "#,
        )
        .bind(project_id)
        .bind(ResourceStatus::Deleted.to_string())
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(resources)
    }
    async fn find_by_id(&self, id: &str) -> Result<Option<Resource>> {
        let resource = sqlx::query_as::<_, Resource>(
            r#"
                SELECT 
                    r.id, 
                	  r.project_id, 
                	  r.name, 
                	  r.kind, 
                	  r.spec, 
                    r.status,
                	  r.created_at, 
                	  r.updated_at
                FROM resource r 
                WHERE r.id = $1 and r.status != $2;
            "#,
        )
        .bind(id)
        .bind(ResourceStatus::Deleted.to_string())
        .fetch_optional(&self.sqlite.db)
        .await?;

        Ok(resource)
    }
    async fn create(&self, resource: &Resource) -> Result<()> {
        let status = resource.status.to_string();

        sqlx::query!(
            r#"
                INSERT INTO resource (
                    id,
                    project_id,
                    name,
                    kind,
                    spec,
                    status,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            resource.id,
            resource.project_id,
            resource.name,
            resource.kind,
            resource.spec,
            status,
            resource.created_at,
            resource.updated_at
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }

    async fn update(&self, resource_update: &ResourceUpdate) -> Result<()> {
        let resource = match self.find_by_id(&resource_update.id).await? {
            Some(resource) => resource,
            None => {
                return Err(Error::Unexpected(format!(
                    "Resource not found: {}",
                    resource_update.id
                )))
            }
        };

        let mut parsed = match serde_json::from_str(&resource.spec) {
            Ok(parsed) => parsed,
            Err(_) => {
                return Err(Error::Unexpected(format!(
                    "Invalid spec found on resource: {}",
                    resource.id
                )))
            }
        };

        json_patch::merge(
            &mut parsed,
            &serde_json::from_str(&resource_update.spec_patch)?,
        );

        let updated_spec = parsed.to_string();
        sqlx::query!(
            r#"
                UPDATE resource 
                SET spec = $1, updated_at = $2
                WHERE id = $3
            "#,
            updated_spec,
            resource_update.updated_at,
            resource.id,
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: &str, deleted_at: &DateTime<Utc>) -> Result<()> {
        let status = ResourceStatus::Deleted.to_string();

        sqlx::query!(
            r#"
                UPDATE resource
                SET 
                    status=$2,
                    updated_at=$3
                WHERE id=$1;
            "#,
            id,
            status,
            deleted_at
        )
        .execute(&self.sqlite.db)
        .await?;

        Ok(())
    }
}

impl FromRow<'_, SqliteRow> for Resource {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        let status: &str = row.try_get("status")?;

        Ok(Self {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            name: row.try_get("name")?,
            kind: row.try_get("kind")?,
            spec: row.try_get("spec")?,
            annotations: None,
            status: status
                .parse()
                .map_err(|err: Error| sqlx::Error::Decode(err.into()))?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::driven::cache::tests::mock_project;

    use super::*;

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
    async fn it_should_return_none_find_project_resources_when_resource_was_deleted() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteResourceDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;

        let resource = Resource {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create(&resource).await.unwrap();
        cache.delete(&resource.id, &Utc::now()).await.unwrap();

        let result = cache.find(&project.id, &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
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

    #[tokio::test]
    async fn it_should_find_resource_by_id() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteResourceDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;

        let resource = Resource {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create(&resource).await.unwrap();

        let result = cache.find_by_id(&resource.id).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
    #[tokio::test]
    async fn it_should_return_none_find_resource_by_id() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteResourceDrivenCache::new(sqlite_cache.clone());

        let result = cache.find_by_id(Default::default()).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
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

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_create_resource_when_project_doesnt_exist() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteResourceDrivenCache::new(sqlite_cache.clone());

        let resource = Resource::default();

        let result = cache.create(&resource).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_delete_resource() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteResourceDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;

        let resource = Resource {
            project_id: project.id.clone(),
            ..Default::default()
        };
        cache.create(&resource).await.unwrap();

        let result = cache.delete(&resource.id, &Utc::now()).await;

        assert!(result.is_ok());
    }
}

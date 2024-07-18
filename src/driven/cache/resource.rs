use anyhow::Result;
use std::sync::Arc;

use crate::domain::resource::{ResourceCache, ResourceDrivenCache};

use super::SqliteCache;

pub struct SqliteResourceCache {
    sqlite: Arc<SqliteCache>,
}
impl SqliteResourceCache {
    pub fn new(sqlite: Arc<SqliteCache>) -> Self {
        Self { sqlite }
    }
}
#[async_trait::async_trait]
impl ResourceDrivenCache for SqliteResourceCache {
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

use anyhow::Result;
use sqlx::{sqlite::SqliteRow, FromRow, Row};
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

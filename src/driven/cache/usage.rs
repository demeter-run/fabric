use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::sync::Arc;

use crate::domain::{
    usage::{cache::UsageDrivenCache, Usage},
    Result,
};

use super::SqliteCache;

pub struct SqliteUsageDrivenCache {
    sqlite: Arc<SqliteCache>,
}
impl SqliteUsageDrivenCache {
    pub fn new(sqlite: Arc<SqliteCache>) -> Self {
        Self { sqlite }
    }
}
#[async_trait::async_trait]
impl UsageDrivenCache for SqliteUsageDrivenCache {
    //async fn find(&self) -> Result<()> {
    //    todo!()
    //}

    async fn create(&self, usages: Vec<Usage>) -> Result<()> {
        let mut tx = self.sqlite.db.begin().await?;

        for usage in usages {
            sqlx::query!(
                r#"
                INSERT INTO usage (
                    id,
                    resource_id,
                    event_id,
                    units,
                    tier,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6)
            "#,
                usage.id,
                usage.resource_id,
                usage.event_id,
                usage.units,
                usage.tier,
                usage.created_at,
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(())
    }
}

impl FromRow<'_, SqliteRow> for Usage {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            event_id: row.try_get("event_id")?,
            resource_id: row.try_get("resource_id")?,
            units: row.try_get("units")?,
            tier: row.try_get("tier")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::driven::cache::tests::{mock_project, mock_resource};

    use super::*;

    #[tokio::test]
    async fn it_should_create_resource() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteUsageDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;
        let resource = mock_resource(sqlite_cache.clone(), &project.id).await;

        let usage = Usage {
            resource_id: resource.id,
            ..Default::default()
        };

        let result = cache.create(vec![usage]).await;

        assert!(result.is_ok());
    }
}

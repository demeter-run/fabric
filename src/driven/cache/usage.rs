use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::sync::Arc;

use crate::domain::{
    usage::{cache::UsageDrivenCache, Usage, UsageReport},
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
    async fn find_report(
        &self,
        project_id: &str,
        page: &u32,
        page_size: &u32,
    ) -> Result<Vec<UsageReport>> {
        let offset = page_size * (page - 1);

        let report = sqlx::query_as::<_, UsageReport>(
            r#"
                SELECT 
                	  r.id as resource_id,
                	  r.kind as resource_kind,
                	  r.spec as resource_spec,
                	  u.tier, 
                	  SUM(u.units) as units, 
                	  STRFTIME('%m-%Y', 'now') as period 
                FROM "usage" u 
                INNER JOIN resource r ON r.id == u.resource_id
                WHERE STRFTIME('%m-%Y', u.created_at) = STRFTIME('%m-%Y', 'now') AND r.project_id = $1 
                GROUP BY resource_id
                ORDER BY units DESC
                LIMIT $2
                OFFSET $3;
            "#,
        )
        .bind(project_id)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(report)
    }

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

impl FromRow<'_, SqliteRow> for UsageReport {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            resource_id: row.try_get("resource_id")?,
            resource_kind: row.try_get("resource_kind")?,
            resource_spec: row.try_get("resource_spec")?,
            units: row.try_get("units")?,
            tier: row.try_get("tier")?,
            period: row.try_get("period")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::driven::cache::tests::{mock_project, mock_resource};

    use super::*;

    #[tokio::test]
    async fn it_should_create_usage() {
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

    #[tokio::test]
    async fn it_should_find_usage_report() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteUsageDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;
        let resource = mock_resource(sqlite_cache.clone(), &project.id).await;

        let usage = Usage {
            resource_id: resource.id,
            ..Default::default()
        };
        cache.create(vec![usage]).await.unwrap();

        let result = cache.find_report(&project.id, &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }
}

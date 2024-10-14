use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::sync::Arc;

use crate::domain::{
    resource::ResourceStatus,
    usage::{cache::UsageDrivenCache, Usage, UsageReport, UsageReportAggregated, UsageResource},
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
                	  r.name as resource_name,
                	  r.spec as resource_spec,
                	  u.tier, 
                    SUM(u.interval) as interval,
                	  SUM(u.units) as units, 
                	  STRFTIME('%m-%Y', 'now') as period 
                FROM "usage" u 
                INNER JOIN resource r ON r.id == u.resource_id
                WHERE STRFTIME('%m-%Y', u.created_at) = STRFTIME('%m-%Y', 'now') AND r.project_id = $1 
                GROUP BY resource_id, tier 
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

    async fn find_report_aggregated(&self, period: &str) -> Result<Vec<UsageReportAggregated>> {
        let report_aggregated = sqlx::query_as::<_, UsageReportAggregated>(
            r#"
                SELECT
                	  p.id as project_id,
                	  p.namespace as project_namespace,
                	  p.billing_provider as project_billing_provider,
                	  p.billing_provider_id as project_billing_provider_id,
                	  r.id as resource_id,
                	  r.kind as resource_kind,
                	  r.name as resource_name,
                	  u.tier as tier, 
                	  SUM(u.interval) as interval, 
                	  SUM(u.units) as units, 
                	  STRFTIME('%m-%Y', 'now') as period 
                FROM "usage" u 
                INNER JOIN resource r ON r.id == u.resource_id
                INNER JOIN project p ON p.id == r.project_id 
                WHERE STRFTIME('%m-%Y', u.created_at) = $1
                GROUP BY resource_id, tier 
                ORDER BY project_namespace, resource_id ASC;
            "#,
        )
        .bind(period)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(report_aggregated)
    }

    async fn find_resouces(&self) -> Result<Vec<UsageResource>> {
        let resources = sqlx::query_as::<_, UsageResource>(
            r#"
                SELECT
                	p.id as project_id,
                	p.namespace as project_namespace,
                	r.id as resource_id,
                	r.name as resource_name,
                	r.spec as resource_spec
                FROM
                	resource r
                INNER JOIN project p ON
                	p.id = r.project_id
                WHERE r.status != $1;
            "#,
        )
        .bind(ResourceStatus::Deleted.to_string())
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(resources)
    }

    async fn create(&self, usages: Vec<Usage>) -> Result<()> {
        let mut tx = self.sqlite.db.begin().await?;

        for usage in usages {
            let interval = usage.interval as i64;
            sqlx::query!(
                r#"
                INSERT INTO usage (
                    id,
                    resource_id,
                    event_id,
                    units,
                    tier,
                    interval,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
                usage.id,
                usage.resource_id,
                usage.event_id,
                usage.units,
                usage.tier,
                interval,
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
        let interval: i64 = row.try_get("interval")?;
        Ok(Self {
            id: row.try_get("id")?,
            event_id: row.try_get("event_id")?,
            resource_id: row.try_get("resource_id")?,
            units: row.try_get("units")?,
            tier: row.try_get("tier")?,
            interval: interval as u64,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl FromRow<'_, SqliteRow> for UsageReport {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            resource_id: row.try_get("resource_id")?,
            resource_kind: row.try_get("resource_kind")?,
            resource_name: row.try_get("resource_name")?,
            resource_spec: row.try_get("resource_spec")?,
            units: row.try_get("units")?,
            tier: row.try_get("tier")?,
            period: row.try_get("period")?,
        })
    }
}

impl FromRow<'_, SqliteRow> for UsageReportAggregated {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        let interval: i64 = row.try_get("interval")?;
        Ok(Self {
            project_id: row.try_get("project_id")?,
            project_namespace: row.try_get("project_namespace")?,
            project_billing_provider: row.try_get("project_billing_provider")?,
            project_billing_provider_id: row.try_get("project_billing_provider_id")?,
            resource_id: row.try_get("resource_id")?,
            resource_kind: row.try_get("resource_kind")?,
            resource_name: row.try_get("resource_name")?,
            tier: row.try_get("tier")?,
            interval: interval as u64,
            units: row.try_get("units")?,
            period: row.try_get("period")?,
        })
    }
}

impl FromRow<'_, SqliteRow> for UsageResource {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            project_id: row.try_get("project_id")?,
            project_namespace: row.try_get("project_namespace")?,
            resource_id: row.try_get("resource_id")?,
            resource_name: row.try_get("resource_name")?,
            resource_spec: row.try_get("resource_spec")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

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

        let usages = vec![
            Usage {
                resource_id: resource.id.clone(),
                ..Default::default()
            },
            Usage {
                resource_id: resource.id.clone(),
                ..Default::default()
            },
        ];

        cache.create(usages).await.unwrap();

        let result = cache.find_report(&project.id, &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }

    #[tokio::test]
    async fn it_should_find_usage_report_after_tier_updated() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteUsageDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;
        let resource = mock_resource(sqlite_cache.clone(), &project.id).await;

        let usages = vec![
            Usage {
                resource_id: resource.id.clone(),
                tier: "0".into(),
                ..Default::default()
            },
            Usage {
                resource_id: resource.id.clone(),
                tier: "1".into(),
                ..Default::default()
            },
        ];

        cache.create(usages).await.unwrap();

        let result = cache.find_report(&project.id, &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 2);
    }

    #[tokio::test]
    async fn it_should_find_usage_report_aggregated() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteUsageDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;
        let resource = mock_resource(sqlite_cache.clone(), &project.id).await;

        let usages = vec![
            Usage {
                resource_id: resource.id.clone(),
                ..Default::default()
            },
            Usage {
                resource_id: resource.id.clone(),
                ..Default::default()
            },
        ];

        cache.create(usages).await.unwrap();

        let result = cache
            .find_report_aggregated(&Utc::now().format("%m-%Y").to_string())
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }

    #[tokio::test]
    async fn it_should_find_resources() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache = SqliteUsageDrivenCache::new(sqlite_cache.clone());

        let project = mock_project(sqlite_cache.clone()).await;
        mock_resource(sqlite_cache.clone(), &project.id).await;

        let result = cache.find_resouces().await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 1);
    }
}

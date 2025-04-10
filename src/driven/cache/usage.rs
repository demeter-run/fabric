use sqlx::{sqlite::SqliteRow, FromRow, Row};
use std::sync::Arc;

use crate::domain::{
    resource::ResourceStatus,
    usage::{
        cache::{UsageDrivenCache, UsageDrivenCacheBackoffice},
        Usage, UsageReport, UsageResource,
    },
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
        cluster_id: Option<String>,
    ) -> Result<Vec<UsageReport>> {
        let offset = page_size * (page - 1);

        let mut query = String::from(
            r#"
                SELECT 
                    u.cluster_id,
                	  p.id as project_id,
                	  p.namespace as project_namespace,
                	  p.billing_provider as project_billing_provider,
                	  p.billing_provider_id as project_billing_provider_id,
                	  r.id as resource_id,
                	  r.kind as resource_kind,
                	  r.name as resource_name,
                	  r.spec as resource_spec,
                	  u.tier, 
                    SUM(u.interval) as interval,
                	  SUM(u.units) as units, 
                	  STRFTIME('%Y-%m', u.created_at) as period
                FROM
                    "usage" u 
                INNER JOIN resource r ON
                    r.id == u.resource_id
                INNER JOIN project p ON
                    p.id == r.project_id 
                WHERE
                    STRFTIME('%Y-%m', u.created_at) = STRFTIME('%Y-%m', 'now')
                    AND r.project_id = $1 
                    --WHERE--
                GROUP BY 
                    u.resource_id,
                    u.tier
                ORDER BY
                    units DESC
                LIMIT $2
                OFFSET $3;
            "#,
        );

        if cluster_id.is_some() {
            query = query.replace("--WHERE--", "AND u.cluster_id = $4");
        }

        let mut query = sqlx::query_as::<_, UsageReport>(&query)
            .bind(project_id)
            .bind(page_size)
            .bind(offset);

        if let Some(cluster_id) = cluster_id {
            query = query.bind(cluster_id);
        }

        let report = query.fetch_all(&self.sqlite.db).await?;

        Ok(report)
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

    async fn find_clusters(
        &self,
        project_id: &str,
        page: &u32,
        page_size: &u32,
    ) -> Result<Vec<String>> {
        let offset = page_size * (page - 1);

        let rows = sqlx::query(
            r#"
                SELECT
                	u.cluster_id,
                	SUM(u.units) as units
                FROM
                	"usage" u
                INNER JOIN resource r ON
	                r.id == u.resource_id
                WHERE
                  STRFTIME('%Y-%m', u.created_at) = STRFTIME('%Y-%m', 'now')
                  AND r.project_id = $1 
                GROUP BY
                	u.cluster_id
                ORDER BY 
                	units DESC
                LIMIT $2
                OFFSET $3;
            "#,
        )
        .bind(project_id)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.sqlite.db)
        .await?;

        let clusters = rows.iter().map(|r| r.get("cluster_id")).collect();
        Ok(clusters)
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
                    cluster_id,
                    units,
                    tier,
                    interval,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
                usage.id,
                usage.resource_id,
                usage.event_id,
                usage.cluster_id,
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
#[async_trait::async_trait]
impl UsageDrivenCacheBackoffice for SqliteUsageDrivenCache {
    async fn find_report_aggregated(
        &self,
        period: &str,
        cluster_id: &str,
    ) -> Result<Vec<UsageReport>> {
        let report_aggregated = sqlx::query_as::<_, UsageReport>(
            r#"
                SELECT
                	u.cluster_id,
                	p.id as project_id,
                	p.namespace as project_namespace,
                	p.billing_provider as project_billing_provider,
                	p.billing_provider_id as project_billing_provider_id,
                	r.id as resource_id,
                	r.kind as resource_kind,
                	r.name as resource_name,
                	r.spec as resource_spec,
                	u.tier as tier,
                	SUM(u.interval) as interval,
                	SUM(u.units) as units,
                	STRFTIME('%Y-%m', u.created_at) as period
                FROM
                	"usage" u
                INNER JOIN resource r ON
                	r.id == u.resource_id
                INNER JOIN project p ON
                	p.id == r.project_id
                WHERE
                	STRFTIME('%Y-%m', u.created_at) = $1
                  AND u.cluster_id = $2
                GROUP BY
                	resource_id,
                	tier
                ORDER BY
                	project_namespace,
                	resource_id ASC;
            "#,
        )
        .bind(period)
        .bind(cluster_id)
        .fetch_all(&self.sqlite.db)
        .await?;

        Ok(report_aggregated)
    }

    async fn find_clusters(&self, period: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            r#"
                SELECT
                	u.cluster_id
                FROM
                	"usage" u
                INNER JOIN resource r ON
	                r.id == u.resource_id
                WHERE
                	STRFTIME('%Y-%m', u.created_at) = $1
                GROUP BY
                	u.cluster_id;
            "#,
        )
        .bind(period)
        .fetch_all(&self.sqlite.db)
        .await?;

        let clusters = rows.iter().map(|r| r.get("cluster_id")).collect();
        Ok(clusters)
    }
}

impl FromRow<'_, SqliteRow> for Usage {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        let interval: i64 = row.try_get("interval")?;
        Ok(Self {
            id: row.try_get("id")?,
            event_id: row.try_get("event_id")?,
            resource_id: row.try_get("resource_id")?,
            cluster_id: row.try_get("cluster_id")?,
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
            cluster_id: row.try_get("cluster_id")?,
            project_id: row.try_get("project_id")?,
            project_namespace: row.try_get("project_namespace")?,
            project_billing_provider: row.try_get("project_billing_provider")?,
            project_billing_provider_id: row.try_get("project_billing_provider_id")?,
            resource_id: row.try_get("resource_id")?,
            resource_kind: row.try_get("resource_kind")?,
            resource_name: row.try_get("resource_name")?,
            resource_spec: row.try_get("resource_spec")?,
            interval: row.try_get("interval")?,
            units: row.try_get("units")?,
            tier: row.try_get("tier")?,
            period: row.try_get("period")?,
            minimum_cost: None,
            units_cost: None,
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

        let result = cache.find_report(&project.id, &1, &12, None).await;

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

        let result = cache.find_report(&project.id, &1, &12, None).await;

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
            .find_report_aggregated(&Utc::now().format("%Y-%m").to_string(), "demeter".into())
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

    #[tokio::test]
    async fn it_should_find_usage_clusters() {
        let sqlite_cache = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let cache: Box<dyn UsageDrivenCache> =
            Box::new(SqliteUsageDrivenCache::new(sqlite_cache.clone()));

        let project = mock_project(sqlite_cache.clone()).await;
        let resource = mock_resource(sqlite_cache.clone(), &project.id).await;

        let usages = vec![
            Usage {
                cluster_id: "cluster_1".into(),
                resource_id: resource.id.clone(),
                ..Default::default()
            },
            Usage {
                cluster_id: "cluster_2".into(),
                resource_id: resource.id.clone(),
                ..Default::default()
            },
        ];

        cache.create(usages).await.unwrap();

        let result = cache.find_clusters(&project.id, &1, &12).await;

        assert!(result.is_ok());
        assert!(result.unwrap().len() == 2);
    }
}

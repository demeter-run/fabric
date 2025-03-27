use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use futures::future::try_join_all;
use tracing::info;
use uuid::Uuid;

use crate::domain::{
    event::{EventDrivenBridge, UsageCreated, UsageUnitCreated},
    Result,
};

use super::{cache::UsageDrivenCache, UsageMetric, UsageResourceUnit, UsageUnitMetric};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait UsageDrivenCluster: Send + Sync {
    async fn find_metrics(
        &self,
        project_name: &str,
        resource_name: &str,
        step: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageResourceUnit>>;
}

pub async fn sync_usage(
    cache: Arc<dyn UsageDrivenCache>,
    usage: Arc<dyn UsageDrivenCluster>,
    event: Arc<dyn EventDrivenBridge>,
    cluster_id: &str,
    step: &str,
    cursor: DateTime<Utc>,
) -> Result<()> {
    let end = Utc::now();

    let resources = cache.find_resouces().await?;

    let mut metrics_map: HashMap<String, UsageMetric> = HashMap::new();
    for r in resources {
        let spec = serde_json::from_str::<serde_json::Value>(&r.resource_spec)?;
        let tier = spec.get("throughputTier");
        if tier.is_none() {
            continue;
        }
        let tier = tier.unwrap().as_str().unwrap();

        let usages = usage
            .find_metrics(&r.project_namespace, &r.resource_name, step, cursor, end)
            .await?;

        if !usages.iter().any(|u| u.tier == tier) {
            let unit = UsageUnitMetric {
                resource_id: r.resource_id.clone(),
                resource_name: r.resource_name.clone(),
                units: 0,
                interval: (end.timestamp() - cursor.timestamp()) as u64,
                tier: tier.into(),
            };
            metrics_map
                .entry(r.project_id.clone())
                .and_modify(|u| u.resources.push(unit.clone()))
                .or_insert(UsageMetric {
                    project_id: r.project_id.clone(),
                    project_namespace: r.project_namespace.clone(),
                    resources: vec![unit],
                });
        }

        for resource_unit in usages {
            if resource_unit.tier != tier && resource_unit.units == 0 {
                continue;
            }

            let unit = UsageUnitMetric {
                resource_id: r.resource_id.clone(),
                resource_name: r.resource_name.clone(),
                units: resource_unit.units,
                interval: resource_unit.interval,
                tier: resource_unit.tier,
            };

            metrics_map
                .entry(r.project_id.clone())
                .and_modify(|u| u.resources.push(unit.clone()))
                .or_insert(UsageMetric {
                    project_id: r.project_id.clone(),
                    project_namespace: r.project_namespace.clone(),
                    resources: vec![unit],
                });
        }
    }

    let tasks = metrics_map.values().map(|u| async {
        let evt = UsageCreated {
            id: Uuid::new_v4().to_string(),
            cluster_id: cluster_id.into(),
            project_id: u.project_id.clone(),
            project_namespace: u.project_namespace.clone(),
            created_at: Utc::now(),
            usages: u
                .resources
                .iter()
                .map(|r| UsageUnitCreated {
                    resource_id: r.resource_id.clone(),
                    resource_name: r.resource_name.clone(),
                    units: r.units,
                    tier: r.tier.clone(),
                    interval: r.interval,
                })
                .collect(),
        };
        event.dispatch(evt.into()).await
    });

    try_join_all(tasks).await?;

    info!(
        cursor = cursor.to_string(),
        end = end.to_string(),
        "usage collected"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        event::MockEventDrivenBridge,
        usage::{cache::MockUsageDrivenCache, UsageResource},
    };

    use super::*;

    #[tokio::test]
    async fn it_should_sync_usage() {
        let mut usage = MockUsageDrivenCluster::new();
        usage
            .expect_find_metrics()
            .return_once(|_, _, _, _, _| Ok(Default::default()));

        let mut cache = MockUsageDrivenCache::new();
        cache
            .expect_find_resouces()
            .return_once(|| Ok(vec![UsageResource::default()]));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let result = sync_usage(
            Arc::new(cache),
            Arc::new(usage),
            Arc::new(event),
            Default::default(),
            Default::default(),
            Default::default(),
        )
        .await;
        assert!(result.is_ok());
    }
}

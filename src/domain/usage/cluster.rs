use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::future::join_all;
use tracing::{error, info};
use uuid::Uuid;

use crate::domain::{
    event::{EventDrivenBridge, UsageCreated, UsageUnitCreated},
    Result,
};

use super::UsageMetric;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait UsageDrivenCluster: Send + Sync {
    async fn find_metrics(
        &self,
        step: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageMetric>>;
}

pub async fn sync_usage(
    usage: Arc<dyn UsageDrivenCluster>,
    event: Arc<dyn EventDrivenBridge>,
    cluster_id: &str,
    step: &str,
    cursor: DateTime<Utc>,
) -> Result<()> {
    let end = Utc::now();

    let usages = usage.find_metrics(step, cursor, end).await?;
    if usages.is_empty() {
        return Ok(());
    }

    let tasks = usages.iter().map(|u| async {
        let evt = UsageCreated {
            id: Uuid::new_v4().to_string(),
            cluster_id: cluster_id.into(),
            project_namespace: u.project_namespace.clone(),
            created_at: Utc::now(),
            usages: u
                .resources
                .iter()
                .map(|r| UsageUnitCreated {
                    resource_name: r.resource_name.clone(),
                    units: r.units,
                    tier: r.tier.clone(),
                    interval: r.interval,
                })
                .collect(),
        };

        if let Err(error) = event.dispatch(evt.into()).await {
            error!(?error, ?u.project_namespace, "fail to dispatch usage event");
        }
    });

    join_all(tasks).await;

    info!(
        cursor = cursor.to_string(),
        end = end.to_string(),
        "usage collected"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::domain::event::MockEventDrivenBridge;

    use super::*;

    #[tokio::test]
    async fn it_should_sync_usage() {
        let mut usage = MockUsageDrivenCluster::new();
        usage
            .expect_find_metrics()
            .return_once(|_, _, _| Ok(Default::default()));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let result = sync_usage(
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

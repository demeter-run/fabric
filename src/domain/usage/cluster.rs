use std::sync::Arc;

use chrono::{DateTime, Utc};
use tracing::info;
use uuid::Uuid;

use crate::domain::{
    event::{EventDrivenBridge, UsageCreated, UsageUnitCreated},
    Result,
};

use super::UsageUnit;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait UsageDrivenCluster: Send + Sync {
    async fn find_metrics(
        &self,
        step: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageUnit>>;
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

    let evt = UsageCreated {
        id: Uuid::new_v4().to_string(),
        cluster_id: cluster_id.into(),
        usages: usages
            .into_iter()
            .map(|u| UsageUnitCreated {
                project_namespace: u.project_namespace,
                resource_name: u.resource_name,
                units: u.units,
                tier: u.tier,
                interval: u.interval,
            })
            .collect(),
        created_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
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

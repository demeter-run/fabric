use std::sync::Arc;

use chrono::{DateTime, Utc};
use tracing::info;
use uuid::Uuid;

use crate::domain::{
    event::{EventDrivenBridge, UsageCreated, UsageUnitCreated},
    Result,
};

use super::UsageUnit;

#[async_trait::async_trait]
pub trait UsageDrivenCluster: Send + Sync {
    async fn find_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageUnit>>;
}

pub async fn sync_usage(
    usage: Arc<dyn UsageDrivenCluster>,
    event: Arc<dyn EventDrivenBridge>,
    cluster_id: &str,
    cursor: DateTime<Utc>,
) -> Result<()> {
    let end = Utc::now();

    let usages = usage.find_metrics(cursor, end).await?;
    if usages.is_empty() {
        return Ok(());
    }

    let evt = UsageCreated {
        id: Uuid::new_v4().to_string(),
        cluster_id: cluster_id.into(),
        usages: usages
            .into_iter()
            .map(|u| UsageUnitCreated {
                resource_id: u.resource_id,
                units: u.units,
                tier: u.tier,
            })
            .collect(),
        created_at: Utc::now(),
    };

    dbg!(&evt);

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
    use mockall::mock;

    use super::*;
    use crate::domain::event::Event;

    mock! {
        pub FakeUsageDriven { }

        #[async_trait::async_trait]
        impl UsageDrivenCluster for FakeUsageDriven {
            async fn find_metrics(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<UsageUnit>>;
        }
    }
    mock! {
        pub FakeEventDrivenBridge { }

        #[async_trait::async_trait]
        impl EventDrivenBridge for FakeEventDrivenBridge {
            async fn dispatch(&self, event: Event) -> Result<()>;
        }
    }

    #[tokio::test]
    async fn it_should_sync_usage() {
        let mut usage = MockFakeUsageDriven::new();
        usage
            .expect_find_metrics()
            .return_once(|_, _| Ok(Default::default()));

        let mut event = MockFakeEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let result = sync_usage(
            Arc::new(usage),
            Arc::new(event),
            Default::default(),
            Default::default(),
        )
        .await;
        assert!(result.is_ok());
    }
}

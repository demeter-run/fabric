use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use tracing::info;
use uuid::Uuid;

use crate::domain::{
    event::{EventDrivenBridge, UsageCreated},
    Result,
};

#[async_trait::async_trait]
pub trait UsageDriven: Send + Sync {
    async fn find_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<HashMap<String, f64>>;
}

pub async fn sync_usage(
    usage: Arc<dyn UsageDriven>,
    event: Arc<dyn EventDrivenBridge>,
    cluster_id: &str,
) -> Result<()> {
    let start = Default::default();
    let end = Default::default();

    let resources = usage.find_metrics(start, end).await?;

    let evt = UsageCreated {
        id: Uuid::new_v4().to_string(),
        cluster_id: cluster_id.into(),
        resources,
        created_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(
        start = start.to_string(),
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
        impl UsageDriven for FakeUsageDriven {
            async fn find_metrics(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<HashMap<String, f64>>;
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

        let result = sync_usage(Arc::new(usage), Arc::new(event), Default::default()).await;
        assert!(result.is_ok());
    }
}

use std::sync::Arc;

use crate::domain::{event::UsageCreated, Result};

use super::Usage;

#[async_trait::async_trait]
pub trait UsageDrivenCache: Send + Sync {
    //async fn find(&self) -> Result<()>;
    async fn create(&self, usage: Vec<Usage>) -> Result<()>;
}

pub async fn create(cache: Arc<dyn UsageDrivenCache>, evt: UsageCreated) -> Result<()> {
    cache.create(evt.into()).await
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;

    mock! {
        pub FakeUsageDrivenCache { }

        #[async_trait::async_trait]
        impl UsageDrivenCache for FakeUsageDrivenCache {
            //async fn find(&self) -> Result<()>;
            async fn create(&self, usage: Vec<Usage>) -> Result<()>;
        }
    }

    #[tokio::test]
    async fn it_should_create_usage_cache() {
        let mut cache = MockFakeUsageDrivenCache::new();
        cache.expect_create().return_once(|_| Ok(()));

        let evt = UsageCreated::default();

        let result = create(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }
}

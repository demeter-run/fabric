use std::sync::Arc;

use crate::domain::{event::UsageCreated, Result};

use super::{Usage, UsageReport};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait UsageDrivenCache: Send + Sync {
    async fn find_report(
        &self,
        project_id: &str,
        page: &u32,
        page_size: &u32,
    ) -> Result<Vec<UsageReport>>;
    async fn create(&self, usage: Vec<Usage>) -> Result<()>;
}

pub async fn create(cache: Arc<dyn UsageDrivenCache>, evt: UsageCreated) -> Result<()> {
    cache.create(evt.into()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn it_should_create_usage_cache() {
        let mut cache = MockUsageDrivenCache::new();
        cache.expect_create().return_once(|_| Ok(()));

        let evt = UsageCreated::default();

        let result = create(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }
}

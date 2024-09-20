use std::sync::Arc;

use crate::domain::{event::UsageCreated, Result};

use super::{Usage, UsageReport, UsageReportAggregated};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait UsageDrivenCache: Send + Sync {
    async fn find_report(
        &self,
        project_id: &str,
        page: &u32,
        page_size: &u32,
    ) -> Result<Vec<UsageReport>>;
    async fn find_report_aggregated(&self, period: &str) -> Result<Vec<UsageReportAggregated>>;
    async fn create(&self, usage: Vec<Usage>) -> Result<()>;
}

pub async fn create(cache: Arc<dyn UsageDrivenCache>, evt: UsageCreated) -> Result<()> {
    cache.create(evt.into()).await
}

pub async fn find_report_aggregated(
    cache: Arc<dyn UsageDrivenCache>,
    period: &str,
) -> Result<Vec<UsageReportAggregated>> {
    cache.find_report_aggregated(period).await
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

    #[tokio::test]
    async fn it_should_find_report_aggregated() {
        let mut cache = MockUsageDrivenCache::new();
        cache
            .expect_find_report_aggregated()
            .return_once(|_| Ok(Default::default()));

        let result = find_report_aggregated(Arc::new(cache), "09-2024").await;
        assert!(result.is_ok());
    }
}

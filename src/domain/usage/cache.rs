use std::sync::Arc;

use futures::future::try_join_all;

use crate::domain::{
    error::Error, event::UsageCreated, resource::cache::ResourceDrivenCache, Result,
};

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

pub async fn create(
    usage_cache: Arc<dyn UsageDrivenCache>,
    resouce_cache: Arc<dyn ResourceDrivenCache>,
    evt: UsageCreated,
) -> Result<()> {
    let tasks = evt
        .usages
        .iter()
        .map(|usage| async {
            let Some(resource) = resouce_cache
                .find_by_name_for_usage(&usage.project_namespace, &usage.resource_name)
                .await?
            else {
                return Err(Error::Unexpected("Resource name has not been found".into()));
            };

            let usage = Usage::from_usage_evt(usage, &resource.id, &evt.id, evt.created_at);
            Ok(usage)
        })
        .collect::<Vec<_>>();

    let usages = try_join_all(tasks).await?;

    usage_cache.create(usages).await
}

pub async fn find_report_aggregated(
    cache: Arc<dyn UsageDrivenCache>,
    period: &str,
) -> Result<Vec<UsageReportAggregated>> {
    cache.find_report_aggregated(period).await
}

#[cfg(test)]
mod tests {
    use crate::domain::resource::{cache::MockResourceDrivenCache, Resource};

    use super::*;

    #[tokio::test]
    async fn it_should_create_usage_cache() {
        let mut usage_cache = MockUsageDrivenCache::new();
        usage_cache.expect_create().return_once(|_| Ok(()));

        let mut resource_cache = MockResourceDrivenCache::new();
        resource_cache
            .expect_find_by_name_for_usage()
            .return_once(|_, _| Ok(Some(Resource::default())));

        let evt = UsageCreated::default();

        let result = create(Arc::new(usage_cache), Arc::new(resource_cache), evt).await;
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

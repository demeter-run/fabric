use std::sync::Arc;

use crate::domain::event::ResourceCreated;

use super::Resource;

use anyhow::Result;

#[async_trait::async_trait]
pub trait ResourceDrivenCache: Send + Sync {
    async fn find(&self, project_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Resource>>;
    async fn create(&self, resource: &Resource) -> Result<()>;
}

pub async fn create(cache: Arc<dyn ResourceDrivenCache>, evt: ResourceCreated) -> Result<()> {
    cache.create(&evt.into()).await
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;

    mock! {
        pub FakeResourceDrivenCache { }

        #[async_trait::async_trait]
        impl ResourceDrivenCache for FakeResourceDrivenCache {
            async fn find(&self,project_id: &str,page: &u32,page_size: &u32) -> Result<Vec<Resource>>;
            async fn create(&self, resource: &Resource) -> Result<()>;
        }
    }

    #[tokio::test]
    async fn it_should_create_resource_cache() {
        let mut cache = MockFakeResourceDrivenCache::new();
        cache.expect_create().return_once(|_| Ok(()));

        let evt = ResourceCreated::default();

        let result = create(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }
}

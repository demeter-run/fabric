use std::sync::Arc;

use crate::domain::{
    event::{ResourceCreated, ResourceDeleted, ResourceUpdated},
    Result,
};

use super::{Resource, ResourceUpdate};

use chrono::{DateTime, Utc};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ResourceDrivenCache: Send + Sync {
    async fn find(&self, project_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Resource>>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Resource>>;
    async fn find_by_name(&self, project_id: &str, name: &str) -> Result<Option<Resource>>;
    async fn find_by_name_for_usage(&self, namespace: &str, name: &str)
        -> Result<Option<Resource>>;
    async fn create(&self, resource: &Resource) -> Result<()>;
    async fn update(&self, resource: &ResourceUpdate) -> Result<()>;
    async fn delete(&self, id: &str, deleted_at: &DateTime<Utc>) -> Result<()>;
}

pub async fn create(cache: Arc<dyn ResourceDrivenCache>, evt: ResourceCreated) -> Result<()> {
    cache.create(&evt.try_into()?).await
}

pub async fn update(cache: Arc<dyn ResourceDrivenCache>, evt: ResourceUpdated) -> Result<()> {
    cache.update(&evt.try_into()?).await
}

pub async fn delete(cache: Arc<dyn ResourceDrivenCache>, evt: ResourceDeleted) -> Result<()> {
    cache.delete(&evt.id, &evt.deleted_at).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn it_should_create_resource_cache() {
        let mut cache = MockResourceDrivenCache::new();
        cache.expect_create().return_once(|_| Ok(()));

        let evt = ResourceCreated::default();

        let result = create(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_delete_resource_cache() {
        let mut cache = MockResourceDrivenCache::new();
        cache.expect_delete().return_once(|_, _| Ok(()));

        let evt = ResourceDeleted::default();

        let result = delete(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }
}

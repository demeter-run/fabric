use std::sync::Arc;

use crate::domain::event::{ProjectCreated, ProjectSecretCreated};
use crate::domain::Result;

use super::{Project, ProjectSecret, ProjectUser};

#[async_trait::async_trait]
pub trait ProjectDrivenCache: Send + Sync {
    async fn find(&self, user_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Project>>;
    async fn find_by_namespace(&self, namespace: &str) -> Result<Option<Project>>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Project>>;
    async fn create(&self, project: &Project) -> Result<()>;
    async fn create_secret(&self, secret: &ProjectSecret) -> Result<()>;
    async fn find_secret_by_project_id(&self, project_id: &str) -> Result<Vec<ProjectSecret>>;
    async fn find_user_permission(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Option<ProjectUser>>;
}

pub async fn create(cache: Arc<dyn ProjectDrivenCache>, evt: ProjectCreated) -> Result<()> {
    cache.create(&evt.try_into()?).await
}

pub async fn create_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    evt: ProjectSecretCreated,
) -> Result<()> {
    cache.create_secret(&evt.into()).await
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;

    mock! {
        pub FakeProjectDrivenCache { }

        #[async_trait::async_trait]
        impl ProjectDrivenCache for FakeProjectDrivenCache {
            async fn find(&self, user_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Project>>;
            async fn find_by_namespace(&self, namespace: &str) -> Result<Option<Project>>;
            async fn find_by_id(&self, id: &str) -> Result<Option<Project>>;
            async fn create(&self, project: &Project) -> Result<()>;
            async fn create_secret(&self, secret: &ProjectSecret) -> Result<()>;
            async fn find_secret_by_project_id(&self, project_id: &str) -> Result<Vec<ProjectSecret>>;
            async fn find_user_permission(&self,user_id: &str, project_id: &str) -> Result<Option<ProjectUser>>;
        }
    }

    #[tokio::test]
    async fn it_should_create_project_cache() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache.expect_create().return_once(|_| Ok(()));

        let evt = ProjectCreated::default();

        let result = create(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_create_project_secret_cache() {
        let mut cache = MockFakeProjectDrivenCache::new();
        cache.expect_create_secret().return_once(|_| Ok(()));

        let evt = ProjectSecretCreated::default();

        let result = create_secret(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }
}

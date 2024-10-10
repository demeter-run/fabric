use chrono::{DateTime, Utc};
use std::sync::Arc;

use crate::domain::event::{
    ProjectCreated, ProjectDeleted, ProjectSecretCreated, ProjectSecretDeleted, ProjectUpdated,
    ProjectUserDeleted, ProjectUserInviteAccepted, ProjectUserInviteCreated,
    ProjectUserInviteDeleted,
};
use crate::domain::Result;

use super::{Project, ProjectSecret, ProjectUpdate, ProjectUser, ProjectUserInvite};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ProjectDrivenCache: Send + Sync {
    async fn find(&self, user_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Project>>;
    async fn find_by_namespace(&self, namespace: &str) -> Result<Option<Project>>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Project>>;
    async fn create(&self, project: &Project) -> Result<()>;
    async fn update(&self, project: &ProjectUpdate) -> Result<()>;
    async fn delete(&self, id: &str, deleted_at: &DateTime<Utc>) -> Result<()>;
    async fn create_secret(&self, secret: &ProjectSecret) -> Result<()>;
    async fn find_secrets(&self, project_id: &str) -> Result<Vec<ProjectSecret>>;
    async fn find_secret_by_id(&self, id: &str) -> Result<Option<ProjectSecret>>;
    async fn delete_secret(&self, id: &str) -> Result<()>;
    async fn find_users(
        &self,
        project_id: &str,
        page: &u32,
        page_size: &u32,
    ) -> Result<Vec<ProjectUser>>;
    async fn find_user_permission(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Option<ProjectUser>>;
    async fn find_user_invites(
        &self,
        project_id: &str,
        page: &u32,
        page_size: &u32,
    ) -> Result<Vec<ProjectUserInvite>>;
    async fn find_user_invite_by_id(&self, id: &str) -> Result<Option<ProjectUserInvite>>;
    async fn find_user_invite_by_code(&self, code: &str) -> Result<Option<ProjectUserInvite>>;
    async fn create_user_invite(&self, invite: &ProjectUserInvite) -> Result<()>;
    async fn create_user_acceptance(&self, invite_id: &str, user: &ProjectUser) -> Result<()>;
    async fn delete_user_invite(&self, invite_id: &str) -> Result<()>;
    async fn delete_user(&self, project_id: &str, id: &str) -> Result<()>;
}

pub async fn create(cache: Arc<dyn ProjectDrivenCache>, evt: ProjectCreated) -> Result<()> {
    cache.create(&evt.try_into()?).await
}

pub async fn update(cache: Arc<dyn ProjectDrivenCache>, evt: ProjectUpdated) -> Result<()> {
    cache.update(&evt.try_into()?).await
}

pub async fn delete(cache: Arc<dyn ProjectDrivenCache>, evt: ProjectDeleted) -> Result<()> {
    cache.delete(&evt.id, &evt.deleted_at).await
}

pub async fn create_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    evt: ProjectSecretCreated,
) -> Result<()> {
    cache.create_secret(&evt.into()).await
}
pub async fn delete_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    evt: ProjectSecretDeleted,
) -> Result<()> {
    cache.delete_secret(&evt.id).await
}

pub async fn create_user_invite(
    cache: Arc<dyn ProjectDrivenCache>,
    evt: ProjectUserInviteCreated,
) -> Result<()> {
    cache.create_user_invite(&evt.try_into()?).await
}

pub async fn delete_user_invite(
    cache: Arc<dyn ProjectDrivenCache>,
    evt: ProjectUserInviteDeleted,
) -> Result<()> {
    cache.delete_user_invite(&evt.id.clone()).await
}

pub async fn create_user_invite_acceptance(
    cache: Arc<dyn ProjectDrivenCache>,
    evt: ProjectUserInviteAccepted,
) -> Result<()> {
    cache
        .create_user_acceptance(&evt.id.clone(), &evt.try_into()?)
        .await
}

pub async fn delete_user(
    cache: Arc<dyn ProjectDrivenCache>,
    evt: ProjectUserDeleted,
) -> Result<()> {
    cache.delete_user(&evt.project_id, &evt.user_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn it_should_create_project_cache() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_create().return_once(|_| Ok(()));

        let evt = ProjectCreated::default();

        let result = create(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_create_project_secret_cache() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_create_secret().return_once(|_| Ok(()));

        let evt = ProjectSecretCreated::default();

        let result = create_secret(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_delete_project_secret_cache() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_delete_secret().return_once(|_| Ok(()));

        let evt = ProjectSecretDeleted::default();

        let result = delete_secret(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_create_user_invite_cache() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_create_user_invite().return_once(|_| Ok(()));

        let evt = ProjectUserInviteCreated::default();

        let result = create_user_invite(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_create_user_invite_acceptance_cache() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_create_user_acceptance()
            .return_once(|_, _| Ok(()));

        let evt = ProjectUserInviteAccepted::default();

        let result = create_user_invite_acceptance(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }
}

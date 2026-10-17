use chrono::{DateTime, Utc};
use std::sync::Arc;

use crate::domain::event::{
    ProjectClaimed, ProjectCreated, ProjectDeleted, ProjectExpired, ProjectOwnerChanged,
    ProjectSecretCreated, ProjectSecretDeleted, ProjectUpdated, ProjectUserDeleted,
    ProjectUserInviteAccepted, ProjectUserInviteCreated, ProjectUserInviteDeleted,
    ProvisionalProjectCreated,
};
use crate::domain::Result;

use super::{
    Project, ProjectClaim, ProjectOwnerChange, ProjectSecret, ProjectUpdate, ProjectUser,
    ProjectUserInvite, ProjectUserProject, ProvisionalProject,
};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ProjectDrivenCache: Send + Sync {
    async fn find(&self, user_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Project>>;
    async fn find_by_namespace(&self, namespace: &str) -> Result<Option<Project>>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Project>>;
    async fn find_by_claim_token_hash(&self, claim_token_hash: &str) -> Result<Option<Project>>;
    async fn find_by_user_code(&self, user_code: &str) -> Result<Option<Project>>;
    async fn create(&self, project: &Project) -> Result<()>;
    async fn create_provisional(&self, project: &ProvisionalProject) -> Result<()>;
    async fn update(&self, project: &ProjectUpdate) -> Result<()>;
    async fn change_owner(&self, change: &ProjectOwnerChange) -> Result<()>;
    async fn delete(&self, id: &str, deleted_at: &DateTime<Utc>) -> Result<()>;
    async fn expire(&self, id: &str, expired_at: &DateTime<Utc>) -> Result<()>;
    async fn claim(&self, claim: &ProjectClaim) -> Result<()>;
    async fn find_expired_provisional(&self, now: &DateTime<Utc>) -> Result<Vec<Project>>;
    async fn count_live_provisional(&self) -> Result<u32>;
    async fn count_registrations_by_source(
        &self,
        source_ip_hash: &str,
        since: &DateTime<Utc>,
    ) -> Result<u32>;
    /// When the registration from `source_ip_hash` at position `offset`, oldest first, among
    /// those created at or after `since`, was created. `None` when there are not that many.
    async fn find_registration_created_at_by_source(
        &self,
        source_ip_hash: &str,
        since: &DateTime<Utc>,
        offset: u32,
    ) -> Result<Option<DateTime<Utc>>>;
    async fn exists_user_code(&self, user_code: &str) -> Result<bool>;
    async fn create_secret(&self, secret: &ProjectSecret) -> Result<()>;
    async fn find_secrets(&self, project: &str) -> Result<Vec<ProjectSecret>>;
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

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ProjectDrivenCacheBackoffice: Send + Sync {
    async fn find_by_user_id(&self, id: &str) -> Result<Vec<Project>>;
    async fn find_by_namespace(&self, namespace: &str) -> Result<Option<Project>>;
    async fn find_by_resource_spec(&self, value: &str) -> Result<Vec<Project>>;
    async fn find_by_resource_name(&self, value: &str) -> Result<Vec<Project>>;
    async fn find_new_users(&self, after: &str) -> Result<Vec<ProjectUserProject>>;
}

pub async fn create(cache: Arc<dyn ProjectDrivenCache>, evt: ProjectCreated) -> Result<()> {
    cache.create(&evt.try_into()?).await
}

pub async fn create_provisional(
    cache: Arc<dyn ProjectDrivenCache>,
    evt: ProvisionalProjectCreated,
) -> Result<()> {
    cache.create_provisional(&evt.try_into()?).await
}

pub async fn update(cache: Arc<dyn ProjectDrivenCache>, evt: ProjectUpdated) -> Result<()> {
    cache.update(&evt.try_into()?).await
}

pub async fn change_owner(
    cache: Arc<dyn ProjectDrivenCache>,
    evt: ProjectOwnerChanged,
) -> Result<()> {
    cache.change_owner(&evt.into()).await
}

pub async fn delete(cache: Arc<dyn ProjectDrivenCache>, evt: ProjectDeleted) -> Result<()> {
    cache.delete(&evt.id, &evt.deleted_at).await
}

pub async fn expire(cache: Arc<dyn ProjectDrivenCache>, evt: ProjectExpired) -> Result<()> {
    cache.expire(&evt.id, &evt.expired_at).await
}

pub async fn claim(cache: Arc<dyn ProjectDrivenCache>, evt: ProjectClaimed) -> Result<()> {
    cache.claim(&evt.into()).await
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
    async fn it_should_create_provisional_project_cache() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_create_provisional()
            .withf(|p| p.project.owner.is_none() && p.project.agent_provisioned)
            .return_once(|_| Ok(()));

        let evt = ProvisionalProjectCreated::default();

        let result = create_provisional(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_expire_project_cache() {
        let evt = ProjectExpired::default();
        let id = evt.id.clone();

        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_expire()
            .withf(move |project_id, _| project_id == id)
            .return_once(|_, _| Ok(()));

        let result = expire(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_claim_project_cache() {
        let evt = ProjectClaimed::default();
        let (id, owner) = (evt.id.clone(), evt.owner.clone());

        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_claim()
            .withf(move |claim| claim.project_id == id && claim.owner == owner)
            .return_once(|_| Ok(()));

        let result = claim(Arc::new(cache), evt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_change_project_owner_cache() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_change_owner().return_once(|_| Ok(()));

        let evt = ProjectOwnerChanged::default();

        let result = change_owner(Arc::new(cache), evt).await;
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

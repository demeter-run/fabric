use std::sync::Arc;

use crate::domain::{
    auth::{assert_project_permission, Credential},
    error::Error,
    project::cache::ProjectDrivenCache,
    Result, PAGE_SIZE_DEFAULT, PAGE_SIZE_MAX,
};

use super::{cache::UsageDrivenCache, UsageReport};

pub async fn fetch_report(
    project_cache: Arc<dyn ProjectDrivenCache>,
    usage_cache: Arc<dyn UsageDrivenCache>,
    cmd: FetchCmd,
) -> Result<Vec<UsageReport>> {
    assert_project_permission(project_cache.clone(), &cmd.credential, &cmd.project_id).await?;

    usage_cache
        .find_report(&cmd.project_id, &cmd.page, &cmd.page_size)
        .await
}

#[derive(Debug, Clone)]
pub struct FetchCmd {
    pub credential: Credential,
    pub project_id: String,
    pub page: u32,
    pub page_size: u32,
}
impl FetchCmd {
    pub fn new(
        credential: Credential,
        project_id: String,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<Self> {
        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(PAGE_SIZE_DEFAULT);

        if page_size >= PAGE_SIZE_MAX {
            return Err(Error::CommandMalformed(format!(
                "page_size exceeded the limit of {PAGE_SIZE_MAX}"
            )));
        }

        Ok(Self {
            credential,
            project_id,
            page,
            page_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use mockall::mock;
    use uuid::Uuid;

    use super::*;
    use crate::domain::{
        project::{Project, ProjectSecret, ProjectUpdate, ProjectUser, ProjectUserInvite},
        usage::Usage,
    };

    mock! {
        pub FakeProjectDrivenCache { }

        #[async_trait::async_trait]
        impl ProjectDrivenCache for FakeProjectDrivenCache {
            async fn find(&self, user_id: &str, page: &u32, page_size: &u32) -> Result<Vec<Project>>;
            async fn find_by_namespace(&self, namespace: &str) -> Result<Option<Project>>;
            async fn find_by_id(&self, id: &str) -> Result<Option<Project>>;
            async fn create(&self, project: &Project) -> Result<()>;
            async fn update(&self, project: &ProjectUpdate) -> Result<()>;
            async fn delete(&self, id: &str, deleted_at: &DateTime<Utc>) -> Result<()>;
            async fn create_secret(&self, secret: &ProjectSecret) -> Result<()>;
            async fn find_secret_by_project_id(&self, project_id: &str) -> Result<Vec<ProjectSecret>>;
            async fn find_user_permission(&self,user_id: &str, project_id: &str) -> Result<Option<ProjectUser>>;
            async fn find_user_invite_by_code(&self, code: &str) -> Result<Option<ProjectUserInvite>>;
            async fn create_user_invite(&self, invite: &ProjectUserInvite) -> Result<()>;
            async fn create_user_acceptance(&self, invite_id: &str, user: &ProjectUser) -> Result<()>;
        }
    }

    mock! {
        pub FakeUsageDrivenCache { }

        #[async_trait::async_trait]
        impl UsageDrivenCache for FakeUsageDrivenCache {
            async fn find_report(&self, project_id: &str, page: &u32, page_size: &u32,) -> Result<Vec<UsageReport>>;
            async fn create(&self, usage: Vec<Usage>) -> Result<()>;
        }
    }

    impl Default for FetchCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                project_id: Uuid::new_v4().to_string(),
                page: 1,
                page_size: 12,
            }
        }
    }

    #[tokio::test]
    async fn it_should_fetch_project_usage_report() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut usage_cache = MockFakeUsageDrivenCache::new();
        usage_cache
            .expect_find_report()
            .return_once(|_, _, _| Ok(vec![UsageReport::default()]));

        let cmd = FetchCmd::default();

        let result = fetch_report(Arc::new(project_cache), Arc::new(usage_cache), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_usage_report_when_user_doesnt_have_permission() {
        let mut project_cache = MockFakeProjectDrivenCache::new();
        project_cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let usage_cache = MockFakeUsageDrivenCache::new();

        let cmd = FetchCmd::default();

        let result = fetch_report(Arc::new(project_cache), Arc::new(usage_cache), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_usage_report_when_secret_doesnt_have_permission() {
        let project_cache = MockFakeProjectDrivenCache::new();
        let usage_cache = MockFakeUsageDrivenCache::new();

        let cmd = FetchCmd {
            credential: Credential::ApiKey(Uuid::new_v4().to_string()),
            ..Default::default()
        };

        let result = fetch_report(Arc::new(project_cache), Arc::new(usage_cache), cmd).await;
        assert!(result.is_err());
    }
}
